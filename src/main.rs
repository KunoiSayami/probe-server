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

use warp::Filter;
use sqlx::Connection;

async fn async_main() -> anyhow::Result<()> {
    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let hello = warp::path!("hello" / String)
        .map(|name| format!("Hello, {}!", name));

    let post_filter = warp::filters::method::post()
        .map(|| {});

    warp::serve(hello)
        .run(([127, 0, 0, 1], 3030))
        .await;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use sqlx::Connection;
    use sqlx::SqliteConnection;
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::query(r#"CREATE TABLE "test" (
	"test"	INTEGER
);"#)
        .execute(&mut conn)
        .await?;

    sqlx::query(r#"INSERT INTO "test"("test") VALUES (?)"#)
        .bind(1)
        .execute(&mut conn)
        .await?;

    let r : (i64,) = sqlx::query_as(r#"select * from "test""#)
        .fetch_one(&mut conn)
        .await?;

    conn.close().await?;

    assert_eq!(r.0, 1);
    Ok(())
}
