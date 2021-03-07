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
use std::path::Path;
use anyhow::Result;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::convert::TryInto;

#[derive(Deserialize, Serialize)]
pub struct Config {
    server: Server,
    telegram: Telegram
}

#[derive(Deserialize, Serialize)]
pub struct Server {
    bind: String,
    port: u16,
    token: String
}

#[derive(Deserialize, Serialize)]
pub struct Telegram {
    bot_token: String,
    api_server: Option<String>,
    owner: i64
}

impl Config {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Config> {
        let contents = std::fs::read_to_string(&path)?;
        let contents_str = contents.as_str();

        Ok(toml::from_str(contents_str)?)
    }

    pub fn get_bind_params(&self) -> Result<([u8; 4], u16)> {
        let addr = Ipv4Addr::from_str(&self.server.bind)?;
        Ok((addr.octets(), self.server.port))
    }
}