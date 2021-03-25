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
#[allow(dead_code)]
pub mod v2 {
    pub const CREATE_TABLES: &str = r#"
    CREATE TABLE "clients" (
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

    CREATE TABLE "pbs_meta" (
        "key"	TEXT NOT NULL,
        "value"	TEXT NOT NULL,
        PRIMARY KEY("key")
    );

    CREATE TABLE "hostname" (
        "id"	INTEGER NOT NULL,
        "name"	TEXT,
        PRIMARY KEY("id")
    );

    INSERT INTO "pbs_meta" VALUES ('version', '2');
    "#;

    pub const VERSION: &str = "2";
}

#[allow(dead_code)]
pub mod v3 {
    pub const CREATE_TABLES: &str = r#"
    CREATE TABLE "clients" (
        "id"	INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        "uuid"	TEXT NOT NULL UNIQUE,
        "boot_time"	INTEGER NOT NULL,
        "last_seen"	INTEGER NOT NULL,
        "hostname"  TEXT,
    );

    CREATE TABLE "raw_data" (
        "id"	INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        "from"	INTEGER NOT NULL,
        "data"	TEXT NOT NULL,
        "timestamp"	INTEGER NOT NULL
    );

    CREATE TABLE "pbs_meta" (
        "key"	TEXT NOT NULL,
        "value"	TEXT NOT NULL,
        PRIMARY KEY("key")
    );

    INSERT INTO "pbs_meta" VALUES ('version', '3');
    "#;

    pub const VERSION: &str = "3";

}
pub use v3::VERSION;
pub use v3 as current;

use serde_derive::{Deserialize, Serialize};

#[derive(sqlx::FromRow, Serialize, Deserialize, Clone, Debug)]
pub struct ClientRow {
    id: i32,
    uuid: String,
    boot_time: u32,
    last_seen: u32,
    hostname: Option<String>
}

#[allow(dead_code)]
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

    pub fn get_hostname(&self) -> &Option<String> {
        &self.hostname
    }
}
