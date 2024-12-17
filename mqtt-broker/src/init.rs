use std::{
    fs::{self, File},
    path::Path,
    process::Command,
    thread::sleep,
    time::Duration,
};

use crate::{config::MqttConfig, logger::BrokerLogger};

pub struct MqttEnv {
    config: MqttConfig,
}

impl MqttEnv {
    pub fn init_env(self) -> Self {
        if self.config.should_log_file() || self.config.should_log_console() {
            let _logger = BrokerLogger::new(&self.config).init().unwrap();
            if self.config.should_log_file() {
                init_log_fs();
            }
        } else {
        }

        init_tls_creds();

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

pub fn init_tls_creds() {
    if !fs::exists("tls").unwrap() {
        fs::create_dir("tls").expect("Failed to create tls directory");
        init_rsa_key();
        init_tls_cert();
    }
}

pub fn init_rsa_key() {
    let pk = Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "RSA",
            "-out",
            "tls/key.pem",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
        ])
        .output();

    while !fs::exists("tls/key.pem").expect("Error opening certificate file") {
        sleep(Duration::from_millis(1));
    }

    if let Err(err) = pk {
        log::error!("Could not create RSA private key: {err}");
        panic!();
    } else if let Ok(pk_out) = pk {
        if !pk_out.status.success() {
            let stderr = String::from_utf8_lossy(&pk_out.stderr);
            log::error!("OpenSSL certificate generation failed: {stderr}");
            panic!();
        }
    }
}

pub fn init_tls_cert() {
    log::info!("Created RSA private key.");

    let cert = Command::new("openssl")
        .args([
            "req",
            "-new",
            "-x509",
            "-key",
            "tls/key.pem",
            "-out",
            "tls/cert.pem",
            "-days",
            "365",
            "-config",
            "openssl.cnf",
        ])
        .output();

    if let Err(err) = cert {
        log::error!("Could not create TLS certificate: {err}");
        panic!();
    } else if let Ok(cert_out) = cert {
        if !cert_out.status.success() {
            let stderr = String::from_utf8_lossy(&cert_out.stderr);
            log::error!("OpenSSL certificate generation failed: {stderr}");
            panic!();
        }
    }

    log::info!("Created TLS certificate.");
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
        let contents = r#"
[connection]
tls = false
ip = "127.0.0.1"
port = 1883
"#;

        fs::write(CONFIG_PATH, contents).expect("Could not create config file");
        log::info!("Initialized new config file.")
    }
}
