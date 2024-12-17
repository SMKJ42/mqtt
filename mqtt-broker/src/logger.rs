use std::{
    fs::{self},
    io::Write,
};

use colored::*;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use time::{format_description::FormatItem, OffsetDateTime};

use crate::config::MqttConfig;

pub struct BrokerLogger {
    write_file: bool,
    write_console: bool,
}

const TIMESTAMP_FORMAT_UTC: &[FormatItem] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
);

impl log::Log for BrokerLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        // TODO: figure out how to get the color strings to be held in a variable evaluated at compile time.
        if self.enabled(record.metadata()) {
            let timestamp = OffsetDateTime::now_utc()
                .format(TIMESTAMP_FORMAT_UTC)
                .expect("Logger could not format the UTC time. It is likely that your system does not support UTC.");

            let colorized_level_string = match record.level() {
                Level::Error => format!("{:<5}", record.level().to_string())
                    .red()
                    .to_string(),
                Level::Warn => format!("{:<5}", record.level().to_string())
                    .yellow()
                    .to_string(),
                Level::Info => format!("{:<5}", record.level().to_string())
                    .cyan()
                    .to_string(),
                Level::Debug => format!("{:<5}", record.level().to_string())
                    .purple()
                    .to_string(),
                Level::Trace => format!("{:<5}", record.level().to_string())
                    .normal()
                    .to_string(),
            };

            if self.write_console {
                self.log_console(record, &colorized_level_string, &timestamp);
            }

            if self.write_file {
                self.log_file(record, &colorized_level_string, &timestamp);
            }
        }
    }

    fn flush(&self) {}
}

impl BrokerLogger {
    fn log_file(&self, record: &Record, colorized_level_string: &str, timestamp: &str) {
        let log_string = format!("{};{};{}\n", record.level(), record.args(), timestamp);

        match record.level() {
            Level::Trace => {
                unimplemented!();
            }
            Level::Debug => match fs::OpenOptions::new().append(true).open("logs/debug.log") {
                Ok(mut file) => {
                    file.write_all(log_string.as_bytes()).unwrap();
                }
                Err(err) => {
                    let err = format!("{colorized_level_string} - Could not write Debug message to logs/debug.log\n\t{err}\n\t\t{}\n\t - {timestamp};", record.args().to_string().split(";").next().unwrap());
                    eprintln!("{}", err);
                    return;
                }
            },
            Level::Error => {
                match fs::OpenOptions::new().append(true).open("logs/error.log") {
                    Ok(mut file) => {
                        file.write_all(log_string.as_bytes()).unwrap();
                    }
                    Err(err) => {
                        let err = format!("{} - Could not write Debug message to logs/error.log\n\t{err}\n\t\t{}\n\t - {timestamp};", Level::Debug.to_string().purple(), record.args().to_string().split(";").next().unwrap());
                        log::debug!("{}", err);
                        eprintln!("{}", err);
                        return;
                    }
                }
                // promote the log to a higher level
            }
            Level::Warn | Level::Info => {
                match fs::OpenOptions::new().append(true).open("logs/main.log") {
                    Ok(mut file) => {
                        file.write_all(log_string.as_bytes()).unwrap();
                    }
                    Err(err) => {
                        let err = format!("{} - Could not write Debug message to logs/main.log\n\t{err}\n\t\t{}\n\t - {timestamp};", Level::Error.as_str().red(), record.args().to_string().split(";").next().unwrap());
                        log::error!("{}", err);
                        eprintln!("{}", err);
                        return;
                    }
                }
            }
        }
    }

    fn log_console(&self, record: &Record, colorized_level_string: &str, timestamp: &str) {
        println!(
            "{colorized_level_string} - {} - {timestamp};",
            record.args(),
        );
    }
}

impl BrokerLogger {
    pub fn new(config: &MqttConfig) -> Self {
        return Self {
            write_file: config.should_log_file(),
            write_console: config.should_log_console(),
        };
    }

    pub fn init(self, level: LevelFilter) -> Result<(), SetLoggerError> {
        log::set_max_level(level);
        log::set_boxed_logger(Box::new(self))
    }
}
