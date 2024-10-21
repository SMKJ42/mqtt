use core::net::Ipv4Addr;

use std::{fs::File, io::Read};

use serde::Deserialize;

// TODO: This will get unwildy really fast... needs some cleanup
pub struct MqttConfig {
    config: ConfigParams,
}

impl MqttConfig {
    pub fn addr(&self) -> String {
        return self.config.connection.ip.to_string()
            + ":"
            + &self.config.connection.port.to_string();
    }

    pub fn is_tls_enabled(&self) -> bool {
        return self.config.connection.tls;
    }
}

impl From<&str> for MqttConfig {
    fn from(value: &str) -> Self {
        let mut file = match File::open(value) {
            Ok(file) => file,
            Err(err) => {
                log::warn!("Could not load file {value} to initialize the configuration.");
                log::error!("{err}");
                panic!();
            }
        };

        let mut buf = String::new();
        if let Err(err) = file.read_to_string(&mut buf) {
            log::warn!("Could not read file {value}");
            log::error!("{err}");
        }
        let config = match ConfigParams::deserialize(toml::Deserializer::new(&buf)) {
            Ok(config) => {
                if config.connection.tls {
                    if config.connection.port == 1883 {
                        log::warn!("Creating TLS connection on port 1883. This port is reserved for Plaintext MQTT connections.");
                    }
                } else if config.connection.port == 8883 {
                    log::warn!("Creating Plaintext connection on port 8883. This port is reserved for TLS MQTT connections.");
                }

                config
            }
            Err(err) => {
                log::warn!("Could not deserialize connection configuration.");
                log::error!("{err}");
                panic!("{err}");
            }
        };

        return Self { config };
    }
}

#[derive(Deserialize)]
pub struct ConfigParams {
    connection: Connection,
}

#[derive(Deserialize)]
struct Connection {
    tls: bool,
    ip: Ipv4Addr,
    port: u16,
}
