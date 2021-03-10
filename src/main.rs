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
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder, HttpMessage};
use log::info;
use sqlx::{Connection, SqliteConnection, Row};
use std::sync::Arc;
use tokio::sync::Mutex;
use futures::StreamExt;
use std::convert::TryInto;
use std::ops::Index;


fn get_current_timestamp() -> u128 {
    let start = std::time::SystemTime::now();
    let since_the_epoch = start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis()
}

async fn query_client_id_from_uuid(mut conn: &SqliteConnection, uuid: &String) -> anyhow::Result<Option<i32>> {
    let r = sqlx::query(r#"SELECT FROM "clients" WHERE "uuid" = ?"#)
        .bind(uuid)
        .fetch_all(&mut conn)
        .await?;
    Ok(if r.is_empty() {
        None
    } else {
        Some(r.index(0).column(0))
    })
}

async fn route_post(_req: HttpRequest, payload: web::Json<structs::Request>, data: web::Data<Arc<Mutex<SqliteConnection>>>) -> actix_web::Result<impl Responder> {
    {
        let conn = data.lock().await;
        let id = query_client_id_from_uuid(&mut (*conn), payload.get_uuid()).await?;
        let id = if id.is_none() {
            if payload.get_action().eq("register") {
                sqlx::query(r#"INSERT INTO "clients" ("uuid", "boot_time", "last_seen") VALUES (?, ?, ?)"#)
                    .bind(payload.get_uuid())
                    .bind(0)
                    .bind(get_current_timestamp().try_into()?)
                    .execute(&mut (*conn))
                    .await?;
            } else {
                return Err(actix_web::error::ErrorBadRequest("Not registered client"))
            }
            query_client_id_from_uuid(&mut (*conn), payload.get_uuid()).await??
        } else {
            id?
        };
        if payload.get_body().is_some() {
            sqlx::query(r#"INSERT INTO "raw_data" ("from", "data", "timestamp") VALUES (?, ?, ?)"#)
                .bind(id)
                .bind(payload.get_body().clone().unwrap())
                .bind(get_current_timestamp().try_into()?)
                .await?;
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

    if rows.len() == 0 {
        sqlx::query(structs::CREATE_TABLES)
            .execute_many(&mut conn)
            .await;
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
                    .route("", web::to(|| HttpResponse::Forbidden())),
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
