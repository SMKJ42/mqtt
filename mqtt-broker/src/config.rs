use core::net::Ipv4Addr;

use std::{fs::File, io::Read, path::Path, str::FromStr};

use log::LevelFilter;
use mqtt_core::qos::QosLevel;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default)]
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

    pub fn tls_enabled(&self) -> bool {
        return self.connection.tls;
    }

    pub fn should_log_file(&self) -> bool {
        return self.logger.file;
    }

    pub fn should_log_console(&self) -> bool {
        return self.logger.console;
    }

    pub fn user_db_connection(&self) -> &str {
        return &self.users.db_connection;
    }

    pub fn require_auth(&self) -> bool {
        return self.users.authenticate;
    }

    pub fn max_queued_messages(&self) -> usize {
        return self.broker.max_queued_messages;
    }

    pub fn default_qos(&self) -> QosLevel {
        return self.broker.default_qos;
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

#[derive(Deserialize, Serialize)]
struct Connection {
    tls: bool,
    ip: Ipv4Addr,
    port: u16,
}

impl Default for Connection {
    fn default() -> Self {
        return Self {
            tls: false,
            ip: Ipv4Addr::new(127, 0, 0, 1),
            port: 1883,
        };
    }
}

#[derive(Deserialize, Serialize)]
pub struct Users {
    authenticate: bool,
    db_connection: String,
}

impl Default for Users {
    fn default() -> Self {
        return Self {
            authenticate: false,
            db_connection: String::from("user.db"),
        };
    }
}

#[derive(Deserialize, Serialize)]
pub struct Logger {
    console: bool,
    file: bool,
    level: String,
}

impl Default for Logger {
    fn default() -> Self {
        return Self {
            console: true,
            file: true,
            level: String::from("trace"),
        };
    }
}

#[derive(Deserialize, Serialize)]
pub struct Broker {
    max_queued_messages: usize,
    default_qos: QosLevel,
}

impl Default for Broker {
    fn default() -> Self {
        return Self {
            max_queued_messages: 128,
            default_qos: QosLevel::ExactlyOnce,
        };
    }
}
