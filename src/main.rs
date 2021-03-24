/*
 ** Copyright (C) 2021 KunoiSayami
 **
 ** This file is part of probe-server and is released under
 ** the AGPL v3 License: https://www.gnu.org/licenses/agpl-3.0.txt
 **
 ** This program is free software: you can redistribute it and/or modify
 ** it under the terms of the GNU Affero General Public License as published by
 ** the Free Software Foundation, either version 3 of the License, or
 ** any later version.
 **
 ** This program is distributed in the hope that it will be useful,
 ** but WITHOUT ANY WARRANTY; without even the implied warranty of
 ** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 ** GNU Affero General Public License for more details.
 **
 ** You should have received a copy of the GNU Affero General Public License
 ** along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

mod configparser;
mod structs;

use crate::configparser::Config;
use crate::structs::{Response, AdditionalInfo};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use log::{debug, info};
use sqlx::{Connection, Row, SqliteConnection};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use teloxide::requests::{Request, Requester, RequesterExt};
use teloxide::types::ParseMode;
use teloxide::Bot;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt as _;

const CLIENT_TIMEOUT: u32 = 15 * 60;
const CLIENT_TIMEOUT_U64: u64 = CLIENT_TIMEOUT as u64;
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const MINIMUM_CLIENT_VERSION: &str = "1.1.0";

fn get_current_timestamp() -> u64 {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
}

struct ExtraData {
    conn: SqliteConnection,
    bot_tx: mpsc::Sender<Command>,
    watchdog_tx: mpsc::Sender<Command>,
}

#[derive(Debug)]
enum Command {
    StringData(String),
    IntegerData(i32),
    Terminate,
}

impl Command {
    fn new<T>(s: T) -> Command
    where
        T: Into<String>,
    {
        Command::StringData(s.into())
    }
}

async fn process_send_message(
    bot: teloxide::adaptors::DefaultParseMode<Bot>,
    owner: i64,
    mut rx: mpsc::Receiver<Command>,
) -> anyhow::Result<()> {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::StringData(text) => {
                bot.send_message(owner, text).send().await?;
            }
            Command::Terminate => break,
            _ => {}
        }
    }
    debug!("Send message daemon exiting...");
    Ok(())
}

async fn route_post(
    _req: HttpRequest,
    payload: web::Json<structs::Request>,
    data: web::Data<Arc<Mutex<ExtraData>>>,
) -> actix_web::Result<HttpResponse> {
    let additional_info: AdditionalInfo = match payload.get_body() {
        None => Default::default(),
        Some(s) => {
            if let Ok(info) = serde_json::from_str::<structs::AdditionalInfo>(s) {
                info
            } else {
                Default::default()
            }
        }
    };
    if payload.get_version().lt(MINIMUM_CLIENT_VERSION) {
        return Err(actix_web::error::ErrorBadRequest(Response::from(structs::ErrorCodes::ClientVersionMismatch)))
    }
    {
        let mut extra_data = data.lock().await;
        let r = sqlx::query(r#"SELECT "id", "boot_time" FROM "clients" WHERE "uuid" = ?"#)
            .bind(payload.get_uuid())
            .fetch_one(&mut extra_data.conn)
            .await;
        let (id, boot_time) = if let Ok(row) =  r {
            (row.get(0), row.get(1))
        } else if payload.get_action().eq("register") {
            sqlx::query(
                r#"INSERT INTO "clients" ("uuid", "boot_time", "last_seen") VALUES (?, ?, ?)"#,
            )
            .bind(payload.get_uuid())
            .bind(additional_info.get_boot_time())
            .bind(get_current_timestamp() as u32)
            .execute(&mut extra_data.conn)
            .await
            .unwrap();
            let r: (i32, i64) = sqlx::query_as(r#"SELECT "id", "boot_time" FROM "clients" WHERE "uuid" = ?"#)
                .bind(payload.get_uuid())
                .fetch_one(&mut extra_data.conn)
                .await
                .unwrap();
            r
        } else {
            return Err(actix_web::error::ErrorBadRequest(Response::from(
                structs::ErrorCodes::NotRegister,
            )));
        };
        match payload.get_action().as_str() {
            "register" => {
                if boot_time != additional_info.get_boot_time() {
                    extra_data
                        .bot_tx
                        .send(Command::new(format!(
                            "<b>{}</b> ({}: <code>{}</code>) comes online",
                            additional_info.get_host_name(),
                            id,
                            payload.get_uuid()
                        )))
                        .await
                        .ok();
                }
                info!("Got register command from {}({})", additional_info.get_host_name(), payload.get_uuid());
            }
            "heartbeat" => {
                debug!("Got heartbeat command from {}({})", id, payload.get_uuid());
                // Update last seen
                sqlx::query(r#"UPDATE "clients" SET "last_seen" = ? WHERE "id" = ? "#)
                    .bind(get_current_timestamp() as u32)
                    .bind(id)
                    .execute(&mut extra_data.conn)
                    .await
                    .unwrap();
                extra_data
                    .watchdog_tx
                    .send(Command::IntegerData(id))
                    .await
                    .ok();

                if payload.get_body().is_some() {
                    sqlx::query(
                        r#"INSERT INTO "raw_data" ("from", "data", "timestamp") VALUES (?, ?, ?)"#,
                    )
                    .bind(id)
                    .bind(payload.get_body().clone().unwrap())
                    .bind(get_current_timestamp() as u32)
                    .execute(&mut extra_data.conn)
                    .await
                    .unwrap();
                }
            }
            _ => return Err(actix_web::error::ErrorBadRequest("Method not allowed")),
        }
    }
    Ok(HttpResponse::Ok().json(Response::new_ok()))
}

async fn route_admin_query(
    _req: HttpRequest,
    payload: web::Json<structs::AdminRequest>,
    _data: web::Data<Arc<Mutex<ExtraData>>>,
) -> actix_web::Result<HttpResponse> {
    debug!("{}", serde_json::to_string(payload.deref()).unwrap());
    Ok(HttpResponse::Ok().json(Response::new_ok()))
}

async fn client_watchdog(
    mut rx: mpsc::Receiver<Command>,
    extra_data: Arc<Mutex<ExtraData>>,
) -> anyhow::Result<()> {
    use Command::*;
    let mut conn = {
        let mut extra = extra_data.lock().await;
        let mut conn_ = SqliteConnection::connect("sqlite::memory:").await?;
        sqlx::query(structs::CREATE_TABLES_WATCHDOG)
            .execute(&mut conn_)
            .await?;
        let r: Vec<(i32,)> = sqlx::query_as(r#"SELECT "id" FROM "clients" WHERE "last_seen" > ?"#)
            .bind((get_current_timestamp() - CLIENT_TIMEOUT_U64) as u32)
            .fetch_all(&mut extra.conn)
            .await?;
        for item in r {
            sqlx::query(r#"INSERT INTO "list" VALUES (?)"#)
                .bind(item.0)
                .execute(&mut conn_)
                .await?;
        }
        conn_
    };
    debug!("Starting watchdog");
    loop {
        if let Ok(Some(cmd)) = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
            match cmd {
                IntegerData(id) => {
                    let items = sqlx::query(r#"SELECT * FROM "list" WHERE "id" = ?"#)
                        .bind(id)
                        .fetch_all(&mut conn)
                        .await?;
                    if items.is_empty() {
                        sqlx::query(r#"INSERT INTO "list" VALUES (?)"#)
                            .bind(id)
                            .execute(&mut conn)
                            .await?;
                    }
                }
                Terminate => break,
                _ => {}
            }
        }
        let current_time = get_current_timestamp() as u32;
        let mut offline_clients: Vec<(i32, String)> = Default::default();
        {
            let mut extras = extra_data.lock().await;
            let mut q = sqlx::query(r#"SELECT * FROM "list""#).fetch(&mut conn);
            while let Some(Ok(row)) = q.next().await {
                let row = sqlx::query_as::<_, structs::ClientRow>(
                    r#"SELECT * FROM "clients" WHERE "id" = ?"#,
                )
                .bind(row.get::<i32, usize>(0))
                .fetch_one(&mut extras.conn)
                .await?;
                if current_time - row.get_last_seen() > CLIENT_TIMEOUT {
                    offline_clients.push((row.get_id(), row.get_uuid().clone()));
                }
            }
            if !offline_clients.is_empty() {
                let uuids: Vec<String> = offline_clients
                    .clone()
                    .into_iter()
                    .map(|x| format!("<code>{}</code>", x.1))
                    .collect();
                extras
                    .bot_tx
                    .send(Command::StringData(format!(
                        "Clients offline:\n{}",
                        uuids.join("\n")
                    )))
                    .await?;
            }
        }
        for client in &offline_clients {
            sqlx::query(r#"DELETE FROM "list" WHERE "id" = ?"#)
                .bind(client.0)
                .execute(&mut conn)
                .await?;
        }
    }
    debug!("Client watchdog exiting...");
    Ok(())
}

async fn async_main() -> anyhow::Result<()> {
    let config = Config::new("data/config.toml")?;

    if !config.get_database_location().eq("sqlite::memory:") {
        let file = std::path::Path::new(config.get_database_location());
        if !file.exists() {
            std::fs::File::create(file)?;
        }
    }

    let mut conn = SqliteConnection::connect(config.get_database_location()).await?;

    let rows = sqlx::query(r#"SELECT name FROM sqlite_master WHERE type='table' AND name=?"#)
        .bind("clients")
        .fetch_all(&mut conn)
        .await?;

    if rows.is_empty() {
        sqlx::query(structs::CREATE_TABLES)
            .execute(&mut conn)
            .await?;
    }

    let bot = Bot::new(config.get_bot_token());
    let bot = match config.get_api_server() {
        Some(api) => bot.set_api_url(api.parse()?),
        None => bot,
    };

    let (bot_tx, bot_rx) = mpsc::channel(1024);
    let (watchdog_tx, watchdog_rx) = mpsc::channel(1024);

    let authorization_guard = crate::configparser::AuthorizationGuard::from(&config);
    let admin_authorization_guard =
        crate::configparser::AuthorizationGuard::from(config.get_admin_token());
    let bind_addr = config.get_bind_params();

    let extra_data = Arc::new(Mutex::new(ExtraData {
        conn,
        bot_tx: bot_tx.clone(),
        watchdog_tx: watchdog_tx.clone(),
    }));
    let guard_task = tokio::spawn(client_watchdog(watchdog_rx, extra_data.clone()));
    let msg_sender = tokio::spawn(process_send_message(
        bot.clone().parse_mode(ParseMode::Html),
        config.get_owner(),
        bot_rx,
    ));

    info!("Bind address: {}", &bind_addr);

    let server = tokio::spawn(
        HttpServer::new(move || {
            App::new()
                .wrap(actix_web::middleware::Logger::default())
                .service(
                    web::scope("/admin")
                        .guard(admin_authorization_guard.to_owned())
                        .data(extra_data.clone())
                        .service(web::resource("").route(web::post().to(route_admin_query)))
                        .route("", web::to(|| HttpResponse::Forbidden())),
                )
                .service(
                    web::scope("/")
                        .guard(authorization_guard.to_owned())
                        .data(extra_data.clone())
                        .route("", web::post().to(route_post)),
                )
                .service(web::scope("/").route(
                    "",
                    web::get().to(|| HttpResponse::Ok().json(Response::new_ok())),
                ))
                .route("/", web::to(|| HttpResponse::Forbidden()))
        })
        .bind(&bind_addr)?
        .run(),
    );

    server.await??;
    bot_tx.send(Command::Terminate).await?;
    watchdog_tx.send(Command::Terminate).await?;
    guard_task.await??;
    msg_sender.await??;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_module("sqlx::query", log::LevelFilter::Warn)
        .init();

    let system = actix::System::new();

    system.block_on(async_main())?;

    system.run()?;

    Ok(())
}
