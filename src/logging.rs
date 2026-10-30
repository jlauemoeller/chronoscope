//! The log file (`chronoscope.log`).
//!
//! A tiny [`log`] backend that appends one line per record to a file. Failing
//! to set it up is never fatal: [`init_logging`] returns an error the caller
//! may ignore.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;

use log::{LevelFilter, Log, Metadata, Record};

struct FileLogger {
    file: Mutex<File>,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\n",
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut file) = self.file.lock() {
            // A failing log write has nowhere to be reported.
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// Creates the parent directory of `log_file` and installs a logger that
/// appends to it. Records up to `Info` level are written.
///
/// Returns an error if the file cannot be opened or a logger is already
/// installed.
pub fn init_logging(log_file: &Path) -> io::Result<()> {
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;
    log::set_boxed_logger(Box::new(FileLogger {
        file: Mutex::new(file),
    }))
    .map_err(io::Error::other)?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
