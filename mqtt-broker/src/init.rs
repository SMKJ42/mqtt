use std::{
    fs::{self, File},
    path::Path,
};

use crate::{config::MqttConfig, logger::BrokerLogger};

pub struct MqttEnv {
    config: MqttConfig,
}

impl MqttEnv {
    pub fn init(self) -> Self {
        if self.config.should_log_file() || self.config.should_log_console() {
            let _logger = BrokerLogger::new(&self.config)
                .init(self.config.log_level())
                .unwrap();
            if self.config.should_log_file() {
                init_log_fs();
            }
        } else {
        }

        return self;
    }

    pub fn new(config_path: &Path) -> Self {
        match fs::exists(config_path) {
            Ok(_true) => {
                if _true {
                } else {
                    init_config();
                }
            }
            Err(err) => {
                panic!("Could not check for config file's existance, {}", err);
            }
        }

        let config = MqttConfig::try_from(config_path).unwrap();

        return Self { config };
    }

    // I dont like that this function takes ownership and moves it to the config. However, after the env is initialized, all we SHOULD need is the config.
    pub fn config(self) -> MqttConfig {
        return self.config;
    }
}

const FILE_CREATE_ERR: &'static str = "Could not create file: ";

pub fn init_log_fs() {
    let path = Path::new("logs");

    if !fs::exists(path).expect("Could not initialize Log files") {
        fs::create_dir(path).expect("Could not create logs directory");

        let path = path.to_path_buf();

        let debug = path.join("debug.log");
        if let Err(err) = File::create(&debug) {
            // debug does not print to the console, so print the error message to the console to provide error information.
            log::error!("{FILE_CREATE_ERR}{}\n\t{err}", debug.display());
        }

        let error = path.join("error.log");
        if let Err(err) = File::create(&error) {
            log::debug!("{FILE_CREATE_ERR}{}\n\t{err}", error.display());
            log::error!("{FILE_CREATE_ERR}{}", error.display());
        }

        let main = path.join("main.log");
        if let Err(err) = File::create(&main) {
            log::debug!("{FILE_CREATE_ERR}{}\n\t{err}", error.display());
            log::error!("{FILE_CREATE_ERR}{}", error.display());
        }

        log::info!("Initialized log directory.")
    }
}

const CONFIG_PATH: &'static str = "config.toml";

pub fn init_config() {
    let config_path = Path::new(CONFIG_PATH);
    if !config_path.exists() {
        let config_content = toml::to_string_pretty(&MqttConfig::default()).unwrap();
        fs::write(CONFIG_PATH, config_content).expect("Could not create config file");
        log::info!("Initialized new config file.")
    }
}
