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
use log::info;
use sqlx::{Connection, Row, SqliteConnection};
use std::sync::Arc;
use tokio::sync::Mutex;

fn get_current_timestamp() -> u128 {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis()
}

async fn route_post(
    _req: HttpRequest,
    payload: web::Json<structs::Request>,
    data: web::Data<Arc<Mutex<SqliteConnection>>>,
) -> actix_web::Result<impl Responder> {
    {
        let mut conn = data.lock().await;
        let r = sqlx::query(r#"SELECT "id" FROM "clients" WHERE "uuid" = ?"#)
            .bind(payload.get_uuid())
            .fetch_one(&mut (*conn))
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
            .execute(&mut (*conn))
            .await
            .unwrap();
            let r: (i32,) = sqlx::query_as(r#"SELECT "id" FROM "clients" WHERE "uuid" = ?"#)
                .bind(payload.get_uuid())
                .fetch_one(&mut (*conn))
                .await
                .unwrap();
            r.0
        } else {
            return Err(actix_web::error::ErrorBadRequest("Not registered client"));
        };
        if payload.get_body().is_some() {
            sqlx::query(r#"INSERT INTO "raw_data" ("from", "data", "timestamp") VALUES (?, ?, ?)"#)
                .bind(id)
                .bind(payload.get_body().clone().unwrap())
                .bind(get_current_timestamp() as u32)
                .execute(&mut (*conn))
                .await
                .unwrap();
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

    let arc_ = Arc::new(Mutex::new(conn));
    let authorization_guard = crate::configparser::AuthorizationGuard::from(&config);
    let bind_addr = config.get_bind_params();

    info!("Bind address: {}", &bind_addr);

    HttpServer::new(move || {
        App::new()
            .service(
                web::scope("/")
                    .guard(authorization_guard.to_owned())
                    .route("", web::to(HttpResponse::Forbidden)),
            )
            .data(arc_.clone())
            .route("/", web::post().to(route_post))
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let system = actix::System::new();

    system.block_on(async_main())?;

    system.run()?;

    Ok(())
}
