use core::net::Ipv4Addr;

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

use log::LevelFilter;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MqttConfig {
    connection: Connection,
    users: Users,
    logger: Logger,
    broker: Broker,
}

impl MqttConfig {
    pub fn addr(&self) -> String {
        return self.connection.ip.to_string() + ":" + &self.connection.port.to_string();
    }

    pub fn is_tls_enabled(&self) -> bool {
        return self.connection.tls;
    }

    pub fn should_log_file(&self) -> bool {
        return self.logger.file;
    }

    pub fn should_log_console(&self) -> bool {
        return self.logger.console;
    }

    pub fn user_db(&self) -> PathBuf {
        match &self.users.user_db_path {
            Some(path) => {
                PathBuf::from_str(&path).expect(&format!("Invalid user database path: {path}"))
            }
            None => PathBuf::from_str("users.db").unwrap(),
        }
    }

    pub fn require_auth(&self) -> bool {
        return self.users.authenticate;
    }

    pub fn max_queued_messages(&self) -> usize {
        return self.broker.max_queued_messages;
    }

    pub fn log_level(&self) -> LevelFilter {
        return LevelFilter::from_str(&self.logger.level).expect(&format!(
            "Invalid log level provided: {}. Accepted levels are: Off, Error, Warn, Info, Debug",
            self.logger.level
        ));
    }
}

impl TryFrom<&Path> for MqttConfig {
    type Error = toml::de::Error;
    fn try_from(value: &Path) -> Result<Self, toml::de::Error> {
        let mut file = match File::open(value) {
            Ok(file) => file,
            Err(err) => {
                log::warn!(
                    "Could not load file: {} to initialize the configuration.",
                    value.to_str().unwrap_or("")
                );
                log::error!("{err}");
                panic!();
            }
        };

        let mut buf = String::new();
        if let Err(err) = file.read_to_string(&mut buf) {
            log::warn!("Could not read file {}", value.to_str().unwrap_or(""));
            log::error!("{err}");
        }

        let config: MqttConfig = toml::from_str(&buf)?;

        // warn for invalid port configurations.
        if config.connection.tls {
            if config.connection.port == 1883 {
                log::warn!("Creating TLS connection on port 1883. This port is reserved for Plaintext MQTT connections.");
            }
        } else if config.connection.port == 8883 {
            log::warn!("Creating Plaintext connection on port 8883. This port is reserved for TLS MQTT connections.");
        }

        // warn for sending plaintext credentials.
        if config.users.authenticate && config.connection.tls == false {
            log::warn!("Requiring client to send credentials in the clear. Please change the configuration if this is not intended.")
        }

        return Ok(config);
    }
}

#[derive(Deserialize)]
struct Connection {
    tls: bool,
    ip: Ipv4Addr,
    port: u16,
}

#[derive(Deserialize)]
pub struct Users {
    authenticate: bool,
    user_db_path: Option<String>,
}

#[derive(Deserialize)]
pub struct Logger {
    console: bool,
    file: bool,
    level: String,
}

#[derive(Deserialize)]
pub struct Broker {
    max_queued_messages: usize,
}
