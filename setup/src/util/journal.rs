use log::{Level, LevelFilter, Log, Metadata, Record};

/// Logs to stderr. Under systemd each line carries a `<N>` syslog priority
/// prefix, which journald turns into the entry's priority.
struct Journal {
    priority_prefix: bool,
}

impl Log for Journal {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if self.priority_prefix {
            eprintln!("<{}>{}", syslog_priority(record.level()), record.args());
        } else {
            eprintln!(
                "{}: {}",
                record.level().as_str().to_lowercase(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

const fn syslog_priority(level: Level) -> u8 {
    match level {
        Level::Error => 3,
        Level::Warn => 4,
        Level::Info => 6,
        Level::Debug | Level::Trace => 7,
    }
}

/// Installs the logger once; later calls keep the first one.
pub fn init() {
    let priority_prefix = std::env::var_os("JOURNAL_STREAM").is_some();
    if log::set_logger(Box::leak(Box::new(Journal { priority_prefix }))).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}
