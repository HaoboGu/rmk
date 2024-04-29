use panic_rtt_target as _;

use log::{Level, LevelFilter, Metadata, Record};
use rtt_target::{rprintln, rtt_init_print};

pub struct Logger {
    level: Level,
}

static LOGGER: Logger = Logger { level: Level::Info };

pub fn init(level: LevelFilter) {
    rtt_init_print!();
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(level))
        .unwrap();
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        let prefix = match record.level() {
            Level::Error => "\x1B[1;31m",
            Level::Warn => "\x1B[1;33m",
            Level::Info => "\x1B[1;32m",
            Level::Debug => "\x1B[1;30m",
            Level::Trace => "\x1B[2;30m",
        };

        rprintln!("{}{} - {}\x1B[0m", prefix, record.level(), record.args());
    }

    fn flush(&self) {}
}
