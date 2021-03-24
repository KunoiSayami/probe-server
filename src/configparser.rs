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
use anyhow::Result;
use serde_derive::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub(crate) server: Server,
    telegram: Telegram,
}

#[derive(Deserialize, Serialize)]
pub struct Server {
    bind: String,
    port: u16,
    pub(crate) token: String,
    database: String,
    admin_token: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Telegram {
    bot_token: String,
    api_server: Option<String>,
    owner: i64,
}

impl Config {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Config> {
        let contents = std::fs::read_to_string(&path)?;
        let contents_str = contents.as_str();

        Ok(toml::from_str(contents_str)?)
    }

    pub fn get_bind_params(&self) -> String {
        format!("{}:{}", self.server.bind, self.server.port)
    }

    pub fn get_bot_token(&self) -> &String {
        &self.telegram.bot_token
    }

    pub fn get_api_server(&self) -> &Option<String> {
        &self.telegram.api_server
    }

    pub fn get_owner(&self) -> i64 {
        self.telegram.owner
    }

    pub fn get_database_location(&self) -> &String {
        &self.server.database
    }
    /*pub fn token_equal(&self, token: &str) -> bool {
        token.eq(&self.server.token)
    }*/

    pub fn get_auth_token(&self) -> &String {
        &self.server.token
    }

    pub fn get_admin_token(&self) -> Option<String> {
        self.server.admin_token.clone()
    }
}


pub mod client {
    use crate::configparser::Config;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Configure {
        pub server: RemoteServer,
        pub statistics: Statistics,
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Statistics {
        pub enabled: bool,
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct RemoteServer {
        pub server_address: String,
        pub token: String,
        pub backup_servers: Option<Vec<String>>,
        pub interval: Option<u32>,
    }

    impl Configure {
        pub fn from_cfg(cfg: &Config, server_address: &str) -> Self {
            Self {
                server: RemoteServer {
                    server_address: server_address.to_string(),
                    token: cfg.get_auth_token().clone(),
                    backup_servers: None,
                    interval: None
                },
                statistics: Statistics { enabled: false }
            }
        }
    }
}