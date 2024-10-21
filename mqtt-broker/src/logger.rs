use std::{
    fs::{self},
    io::Write,
};

use colored::*;
use log::{Level, Metadata, Record, SetLoggerError};
use time::{format_description::FormatItem, OffsetDateTime};

pub struct BrokerLoger;

const TIMESTAMP_FORMAT_UTC: &[FormatItem] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
);

impl log::Log for BrokerLoger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
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

            let timestamp = OffsetDateTime::now_utc()
                .format(TIMESTAMP_FORMAT_UTC)
                .expect("Logger could not format the UTC time. It is likely that your system does not support UTC.");

            let log_string = format!("{};{};{}\n", record.level(), record.args(), timestamp);

            // TODO: More graceful error handling.
            match record.level() {
                Level::Trace => {
                    unimplemented!();
                }
                Level::Debug => {
                    if let Ok(mut file) = fs::OpenOptions::new().append(true).open("logs/debug.log")
                    {
                        file.write_all(log_string.as_bytes()).unwrap();
                        // we dont want to print debug output, so return.
                        return;
                    }
                    // Cant promote the log, so print to the terminal.
                    else {
                        let err = format!(
                            "{colorized_level_string} - Could not write Debug message to logs/debug.log\n\t{} - {timestamp};",
                            record.args(),
                        );
                        println!("{}", err);
                        return;
                    }
                }
                Level::Error => {
                    if let Ok(mut file) = fs::OpenOptions::new().append(true).open("logs/error.log")
                    {
                        file.write_all(log_string.as_bytes()).unwrap();
                    }
                    // promote the log to a higher level
                    else {
                        let err = format!(
                            "{colorized_level_string} - Could not write Debug message to logs/debug.log\n\t{} - {timestamp};",
                            record.args(),
                        );
                        log::debug!("{}", err);
                        println!("{}", err);
                        return;
                    }
                }
                Level::Warn | Level::Info => {
                    if let Ok(mut file) = fs::OpenOptions::new().append(true).open("logs/main.log")
                    {
                        file.write_all(log_string.as_bytes()).unwrap();
                    }
                    // promote the log to a higher level
                    else {
                        let err = format!(
                            "{colorized_level_string} - Could not write Debug message to logs/debug.log\n\t{} - {timestamp};",
                            record.args(),
                        );
                        log::error!("{}", err);
                        return;
                    }
                }
            }

            // error messages are delimeted by a ; char
            println!(
                "{colorized_level_string} - {} - {timestamp};",
                record.args(),
            );
        }
    }

    fn flush(&self) {}
}

impl BrokerLoger {
    pub fn new() -> Self {
        return Self;
    }

    pub fn init(self) -> Result<(), SetLoggerError> {
        log::set_max_level(log::LevelFilter::Info);
        log::set_boxed_logger(Box::new(self))
    }
}
