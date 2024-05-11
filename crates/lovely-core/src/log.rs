use std::sync::RwLock;
use std::path::Path;
use std::io::Write;
use std::fs::{self, File};

use chrono::Local;

// Exports for convenience.
pub use log::{info, error, warn, debug, trace, LevelFilter};

use log::{Level, Log, Metadata, Record, SetLoggerError};
use once_cell::sync::OnceCell;

static LOGGER: OnceCell<LovelyLogger> = OnceCell::new();

struct LovelyLogger {
    use_console: bool,
    log_file: RwLock<File>,
}

impl Log for LovelyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        // An annoying hack to differentiate between lovely and game logging.
        let args = format!("{}", record.args());
        let msg = if args.starts_with("[G]") {
            format!("{} - {}", record.level(), record.args())
        } else {
            format!("{} - [♥] {}", record.level(), record.args())
        };
                
        if self.enabled(record.metadata()) && self.use_console {
            println!("{msg}");
        }

        // Append the line to the log file, creating it if it does not exist.
        let mut file = self.log_file.write().unwrap();
        file.write_all(msg.as_bytes()).unwrap();
        file.write_all("\n".as_bytes()).unwrap();
    }

    fn flush(&self) {}
}

pub fn init(log_dir: &Path) -> Result<(), SetLoggerError> {
    // We create a log file within the log directory of name lovely-datetime.log
    if !log_dir.is_dir() {
        fs::create_dir_all(log_dir).unwrap();
    }

    let now = Local::now();
    let timestamp = now.format("%Y.%m.%d-%H.%M.%S");
    
    let log_name = format!("lovely-{timestamp}.log");
    let log_path = log_dir.join(log_name);
    let log_file = File::create(&log_path)
        .unwrap_or_else(|e| panic!("Failed to create log file at {log_path:?}: {e}"));

    let logger = LovelyLogger {
        use_console: true,
        log_file: RwLock::new(log_file),
    };
    
    log::set_logger(LOGGER.get_or_init(|| logger)).map(|_| log::set_max_level(LevelFilter::Info))
}
