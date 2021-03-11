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
use actix_web::dev::RequestHead;
use actix_web::guard::Guard;
use anyhow::Result;
use serde_derive::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize)]
pub struct Config {
    server: Server,
    telegram: Telegram,
}

#[derive(Deserialize, Serialize)]
pub struct Server {
    bind: String,
    port: u16,
    token: String,
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

    /*pub fn token_equal(&self, token: &str) -> bool {
        token.eq(&self.server.token)
    }*/
}

#[derive(Clone)]
pub struct AuthorizationGuard {
    token: String,
}

impl From<&Config> for AuthorizationGuard {
    fn from(cfg: &Config) -> Self {
        AuthorizationGuard {
            token: format!("Bearer {}", &cfg.server.token).trim().to_string(),
        }
    }
}

impl Guard for AuthorizationGuard {
    fn check(&self, request: &RequestHead) -> bool {
        if let Some(val) = request.headers.get("authorization") {
            return val != &self.token;
        }
        true
    }
}
