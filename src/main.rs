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
use std::future::Future;
use std::sync::Arc;
use teloxide::requests::{Request, ResponseResult};
use teloxide::Bot;
use tokio::sync::{mpsc, Mutex};
use tokio_compat_02::FutureExt as _;
use teloxide::types::ParseMode;

fn get_current_timestamp() -> u128 {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis()
}

struct ExtraData {
    conn: SqliteConnection,
    tx: mpsc::Sender<Command>,
}

#[derive(Debug)]
enum Command {
    Data(String),
    Terminate,
}

impl Command {
    fn new<T>(s: T) -> Command
    where
        T: Into<String>,
    {
        Command::Data(s.into())
    }
}

async fn process_send_message(
    bot: Bot,
    owner: i64,
    mut rx: mpsc::Receiver<Command>,
) -> anyhow::Result<()> {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::Data(text) => {
                bot.send_message(owner, text).send().compat().await?;
            }
            Command::Terminate => break,
        }
    }
    Ok(())
}

async fn route_post(
    _req: HttpRequest,
    payload: web::Json<structs::Request>,
    data: web::Data<Arc<Mutex<ExtraData>>>,
) -> actix_web::Result<impl Responder> {
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
            return Err(actix_web::error::ErrorBadRequest("Not registered client"));
        };
        match payload.get_action().as_str() {
            "register" => {
                extra_data.tx.send(Command::new(format!("{} (<code>{}</code>) comes online", id, payload.get_uuid()))).await.ok();
            },
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
            _ => {
                return Err(actix_web::error::ErrorBadRequest("Method not allowed"))
            }

        }
    }
    Ok(HttpResponse::Ok().json(Response::new_ok()))
}

async fn async_main() -> anyhow::Result<()> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;

    let rows = sqlx::query(r#"SELECT name FROM sqlite_master WHERE type='table' AND name='?'"#)
        .bind("clients")
        .fetch_all(&mut conn)
        .await?;

    if rows.is_empty() {
        sqlx::query(structs::CREATE_TABLES)
            .execute(&mut conn)
            .await?;
    }

    let config = Config::new("data/config.toml")?;

    let bot = Bot::builder()
        .token(config.get_bot_token())
        .parse_mode(ParseMode::HTML)
        .build();

    let (tx, mut rx) = mpsc::channel(1024);

    let extra_data = Arc::new(Mutex::new(ExtraData {
        conn,
        tx: tx.clone(),
    }));
    let authorization_guard = crate::configparser::AuthorizationGuard::from(&config);
    let bind_addr = config.get_bind_params();

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

    let msg_sender = tokio::spawn(process_send_message(bot.clone(), config.get_owner(), rx));
    server.await??;
    tx.send(Command::Terminate).await?;
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
