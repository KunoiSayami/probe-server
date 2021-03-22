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
use crate::structs::Response;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{debug, info};
use sqlx::{Connection, Row, SqliteConnection};
use std::sync::Arc;
use std::time::Duration;
use teloxide::requests::{Request, Requester, RequesterExt};
use teloxide::types::ParseMode;
use teloxide::Bot;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt as _;

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
) -> actix_web::Result<impl Responder> {
    let hostname: String = match payload.get_body() {
        None => Default::default(),
        Some(s) => {
            if let Ok(info) = serde_json::from_str::<structs::AdditionalInfo>(s) {
                info.get_host_name().clone()
            } else {
                Default::default()
            }
        }
    };
    {
        let mut extra_data = data.lock().await;
        let r = sqlx::query(r#"SELECT "id" FROM "clients" WHERE "uuid" = ?"#)
            .bind(payload.get_uuid())
            .fetch_one(&mut extra_data.conn)
            .await;
        let id = if r.is_ok() {
            r.unwrap().get(0)
        } else if payload.get_action().eq("register") {
            sqlx::query(
                r#"INSERT INTO "clients" ("uuid", "boot_time", "last_seen") VALUES (?, ?, ?)"#,
            )
            .bind(payload.get_uuid())
            .bind(0u32)
            .bind(get_current_timestamp() as u32)
            .execute(&mut extra_data.conn)
            .await
            .unwrap();
            let r: (i32,) = sqlx::query_as(r#"SELECT "id" FROM "clients" WHERE "uuid" = ?"#)
                .bind(payload.get_uuid())
                .fetch_one(&mut extra_data.conn)
                .await
                .unwrap();
            r.0
        } else {
            return Err(actix_web::error::ErrorBadRequest(Response::from(
                structs::ErrorCodes::NotRegister,
            )));
        };
        match payload.get_action().as_str() {
            "register" => {
                extra_data
                    .bot_tx
                    .send(Command::new(format!(
                        "<b>{}</b> ({}: <code>{}</code>) comes online",
                        id,
                        hostname,
                        payload.get_uuid()
                    )))
                    .await
                    .ok();
                extra_data
                    .watchdog_tx
                    .send(Command::IntegerData(id))
                    .await
                    .ok();
            }
            "heartbeat" => {
                // Update last seen
                sqlx::query(r#"UPDATE "clients" SET "lastseen" = ? WHERE "id" = ? "#)
                    .bind(get_current_timestamp() as u32)
                    .bind(id)
                    .execute(&mut extra_data.conn)
                    .await
                    .unwrap();

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
            .bind((get_current_timestamp() - 300) as u32)
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
            let mut q = sqlx::query(r#"SELECT * FROM "list""#)
                .fetch(&mut conn);
            while let Some(Ok(row)) = q.next().await
            {
                let row = sqlx::query_as::<_, structs::ClientRow>(
                    r#"SELECT * FROM "clients" WHERE "id" = ?"#,
                )
                .bind(row.get::<i32, usize>(0))
                .fetch_one(&mut extras.conn)
                .await?;
                if current_time - row.get_last_seen() > 300 {
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

    let file = std::path::Path::new(config.get_database_location());
    if !file.exists() {
        std::fs::File::create(file)?;
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
                .service(
                    web::scope("/")
                        .guard(authorization_guard.to_owned())
                        .route("", web::to(HttpResponse::Forbidden)),
                )
                .data(extra_data.clone())
                .route("/", web::post().to(route_post))
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
    env_logger::init();

    let system = actix::System::new();

    system.block_on(async_main())?;

    system.run()?;

    Ok(())
}
