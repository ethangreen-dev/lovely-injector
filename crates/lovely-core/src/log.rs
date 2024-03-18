// Exports for convenience.
pub use log::{info, error, warn, debug, trace, LevelFilter};

use log::{Level, Log, Metadata, Record, SetLoggerError};

use crate::hud::MSG_TX;

static LOGGER: LovelyLogger = LovelyLogger {
    use_console: false,
    use_hud: true,
};

struct LovelyLogger {
    use_console: bool,
    use_hud: bool,
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

        if self.use_hud {
            MSG_TX
                .get()
                .unwrap()
                .send(msg)
                .expect("Failed to pump hud log message");
        }
    }

    fn flush(&self) {}
}

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|_| log::set_max_level(LevelFilter::Info))
}
