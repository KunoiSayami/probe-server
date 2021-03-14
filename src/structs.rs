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
use serde_derive::{Deserialize, Serialize};

pub const CREATE_TABLES: &str = r#"CREATE TABLE "clients" (
	"id"	INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
	"uuid"	TEXT NOT NULL UNIQUE,
	"boot_time"	INTEGER NOT NULL,
	"last_seen"	INTEGER NOT NULL
);

CREATE TABLE "raw_data" (
	"id"	INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
	"from"	INTEGER NOT NULL,
	"data"	TEXT NOT NULL,
	"timestamp"	INTEGER NOT NULL
);
"#;

pub const CREATE_TABLES_WATCHDOG: &str = r#"CREATE TABLE "list" (
    "id"    INTEGER NOT NULL UNIQUE,
)
"#;

#[derive(Deserialize, Serialize)]
pub struct Response {
    status: i64,
    error_code: Option<i64>,
    message: Option<String>,
}

impl Response {
    pub fn new(status: i64, error_code: Option<i64>, message: Option<String>) -> Response {
        Response {
            status,
            error_code,
            message,
        }
    }

    pub fn new_ok() -> Response {
        Response {
            status: 200,
            error_code: None,
            message: None,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Request {
    action: String,
    uuid: String,
    body: Option<String>,
}

impl Request {
    pub fn get_action(&self) -> &String {
        &self.action
    }

    pub fn get_uuid(&self) -> &String {
        &self.uuid
    }

    pub fn get_body(&self) -> &Option<String> {
        &self.body
    }
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AdditionalInfo {
    hostname: Option<String>,
    boot_time: Option<String>,
}