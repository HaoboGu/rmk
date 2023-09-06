use panic_rtt_target as _;

use log::{Level, LevelFilter, Metadata, Record};
use rtt_target::{rprintln, rtt_init_print};

pub struct Logger {
    level: Level,
}

static LOGGER: Logger = Logger { level: Level::Info };

pub fn init() {
    rtt_init_print!();
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        rprintln!("{} - {}", record.level(), record.args());
    }

    fn flush(&self) {}
}
