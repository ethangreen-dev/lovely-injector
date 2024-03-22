// Exports for convenience.
pub use log::{info, error, warn, debug, trace, LevelFilter};

use log::{Level, Log, Metadata, Record, SetLoggerError};

static LOGGER: LovelyLogger = LovelyLogger {
    use_console: true,
};

struct LovelyLogger {
    use_console: bool,
}

impl Log for LovelyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let msg = format!("{} - {}", record.level(), record.args());
        if self.use_console {
            println!("{msg}");
        }
    }

    fn flush(&self) {}
}

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|_| log::set_max_level(LevelFilter::Info))
}
