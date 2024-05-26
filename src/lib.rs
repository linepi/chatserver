pub mod common;
pub mod client;
pub mod server;

pub mod chat {
    tonic::include_proto!("chat");
}

use log::{Record, Level, Metadata};
use log::{SetLoggerError, LevelFilter};
use colored::Colorize;

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn log_init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
}

impl std::fmt::Display for chat::Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let username = &self.client.as_ref().unwrap().user.as_ref().unwrap().name;
        // let milli =  self.time;
        let msg = String::from_utf8(self.bytes.clone()).unwrap();
        // write!(f, "[{}] {}: {}", common::human_milli_seconds(milli), username, msg)?;
        write!(f, "{}: {}", username.green().bold(), msg)?;
        Ok(())
    } 
}

impl chat::Room {
    pub fn from_file(filepath: &String) -> Result<Self, Box<dyn std::error::Error>> {
        let buf = std::fs::read(filepath)?;
        Ok(prost::Message::decode(&buf[..]).unwrap())
    }

    pub fn to_file(&self, filepath: &String) -> Result<(), Box<dyn std::error::Error>> {
        use prost::Message;
        let mut buf = vec![];
        self.encode(&mut buf)?;
        std::fs::write(filepath, buf)?;
        Ok(())
    }
}

impl chat::User {
    pub fn from_file(filepath: &String) -> Result<Self, Box<dyn std::error::Error>> {
        let buf = std::fs::read(filepath)?;
        Ok(prost::Message::decode(&buf[..]).unwrap())
    }

    pub fn to_file(&self, filepath: &String) -> Result<(), Box<dyn std::error::Error>> {
        use prost::Message;
        let mut buf = vec![];
        self.encode(&mut buf)?;
        std::fs::write(filepath, buf)?;
        Ok(())
    }
}

impl chat::Client {
    pub fn username(&self) -> String {
        self.user.as_ref().unwrap().name.clone()
    }
}
