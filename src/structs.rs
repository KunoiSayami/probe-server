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
#![allow(dead_code)]
use serde_derive::{Deserialize, Serialize};
use std::fmt::Formatter;

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
    "id"    INTEGER NOT NULL PRIMARY KEY
);
"#;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Response {
    status: i64,
    #[deprecated(since = "0.2.2")]
    error_code: Option<i64>,
    message: Option<String>,
}

impl Response {
    pub fn new(status: i64, message: Option<String>) -> Response {
        Response {
            status,
            message,
            ..Default::default()
        }
    }

    pub fn new_ok() -> Response {
        Response {
            status: 200,
            message: None,
            ..Default::default()
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
    hostname: String,
    boot_time: i64,
}

impl AdditionalInfo {
    pub fn get_host_name(&self) -> &String {
        &self.hostname
    }

    pub fn get_boot_time(&self) -> i64 {
        self.boot_time
    }
}

#[derive(sqlx::FromRow)]
pub struct ClientRow {
    id: i32,
    uuid: String,
    boot_time: u32,
    last_seen: u32,
}

impl ClientRow {
    pub fn get_id(&self) -> i32 {
        self.id
    }

    pub fn get_uuid(&self) -> &String {
        &self.uuid
    }

    pub fn get_boot_time(&self) -> u32 {
        self.boot_time
    }

    pub fn get_last_seen(&self) -> u32 {
        self.last_seen
    }
}

#[derive(Debug, Clone)]
pub enum ErrorCodes {
    OK,
    NotRegister,
}

impl std::fmt::Display for ErrorCodes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorCodes::OK => "",
                ErrorCodes::NotRegister => "Not registered client",
            }
        )
    }
}

impl From<ErrorCodes> for Response {
    fn from(err_codes: ErrorCodes) -> Self {
        Self::from(&err_codes)
    }
}

impl From<&ErrorCodes> for Response {
    fn from(err_codes: &ErrorCodes) -> Self {
        match err_codes {
            ErrorCodes::OK => Self::new_ok(),
            _ =>
                Self::new(
                    400,
                    Option::from(err_codes.to_string()),
                )
        }
    }
}
impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}
