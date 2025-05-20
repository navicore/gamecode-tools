use log::{debug, info, trace, LevelFilter};

/// Test that logging can be configured and used
#[test]
#[allow(dead_code)]
fn test_logging_works() {
    // Setup a simple logger that writes to memory
    let _logs = test_logger::setup(LevelFilter::Debug);

    // Log messages at different levels
    trace!("This is a trace message");
    debug!("This is a debug message");
    info!("This is an info message");

    // Debug and info should be visible, trace should not
    let logs = test_logger::logs();
    assert!(!logs.contains("This is a trace message"));
    assert!(logs.contains("This is a debug message"));
    assert!(logs.contains("This is an info message"));
}

// Simple logger implementation for testing
mod test_logger {
    use log::{Level, LevelFilter, Metadata, Record};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static LOGGER: TestLogger = TestLogger;
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    static LOGS: Mutex<String> = Mutex::new(String::new());

    struct TestLogger;

    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Debug
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) {
                let mut log_buffer = LOGS.lock().unwrap();
                let msg = format!("{} - {}\n", record.level(), record.args());
                log_buffer.push_str(&msg);
            }
        }

        fn flush(&self) {}
    }

    pub fn setup(level: LevelFilter) -> bool {
        if INITIALIZED.load(Ordering::Relaxed) {
            return true;
        }

        if log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(level))
            .is_err()
        {
            return false;
        }

        INITIALIZED.store(true, Ordering::Relaxed);
        true
    }

    pub fn logs() -> String {
        LOGS.lock().unwrap().clone()
    }
}
