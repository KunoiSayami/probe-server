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

use warp::Filter;
use sqlx::{Connection, SqliteConnection};
use std::time::Duration;
use crate::configparser::Config;

async fn async_main() -> anyhow::Result<()> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;

    let mut rows = sqlx::query(r#"SELECT name FROM sqlite_master WHERE type='table' AND name='?'"#)
        .bind("clients")
        .fetch_all(&mut conn)
        .await?;

    if rows.len() == 0 {
        sqlx::query(structs::CREATE_TABLES)
            .execute(&mut conn)
            .await?;
    }

    let config = Config::new("data/config.toml")?;


    let route = warp::filters::method::post()
        .and(warp::body::json())
        .and(warp::filters::header::optional("authorization"))
        .map(|body: serde_json::Value, token: Option<String>| {
            if let Some(token) = token {
                println!("{}", token);
            }
            warp::reply::json(&structs::Response::new_ok())
        });

    let (tx, rx) = tokio::sync::oneshot::channel();
    let (addr, server) = warp::serve(route)
        .bind_with_graceful_shutdown(config.get_bind_params()?, async {rx.await.ok();
        });


    tokio::task::spawn(server);
    tx.send(()).unwrap();
    conn.close().await?;
    Ok(())
}


fn main() -> anyhow::Result<()>{

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main())?;
    Ok(())
}