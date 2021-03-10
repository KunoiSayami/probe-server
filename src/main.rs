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
use sqlx::{Connection, SqliteConnection};
use std::sync::Arc;
use tokio::sync::Mutex;
use futures::StreamExt;

const CHUNK_MAX_SIZE: usize = 262_144;

async fn route_post(mut req: HttpRequest, mut payload: web::Payload, data: web::Data<Arc<Mutex<SqliteConnection>>>) -> Result<impl Responder, actix_web::Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > CHUNK_MAX_SIZE {
            return Err(actix_web::error::ErrorBadRequest("overflow"))
        }
        body.extend_from_slice(&chunk);
    }
    info!("{}", String::from_utf8_lossy(&body));
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
