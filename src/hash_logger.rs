use std::{
    fs::OpenOptions,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use once_cell::sync::Lazy;

// Static flag to control logging
static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

pub struct HashLogger {
    file: Arc<Mutex<Option<std::fs::File>>>,
}

impl HashLogger {
    fn new() -> Self {
        Self {
            file: Arc::new(Mutex::new(None)),
        }
    }

    fn get_instance() -> &'static HashLogger {
        static INSTANCE: Lazy<HashLogger> = Lazy::new(HashLogger::new);
        &INSTANCE
    }

    pub fn init() {
        match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("hashes.log") {
            Ok(file) => {
                let instance = Self::get_instance();
                let mut file_guard = instance.file.lock().unwrap();
                *file_guard = Some(file);
                LOGGING_ENABLED.store(true, Ordering::SeqCst);
            }
            Err(e) => {
                eprintln!("ERROR: Failed to open hash log file: {}", e);
            }
        }
    }

    pub fn log_hash(nonce: u32, hash_value: u64, difficulty: u64, job_id: &str) {
        if !LOGGING_ENABLED.load(Ordering::Relaxed) {
            return;
        }

        let instance = Self::get_instance();
        let file_guard = instance.file.lock().unwrap();
        if let Some(mut file) = file_guard.as_ref() {
            if let Err(e) = writeln!(file, "{},{},{},{}", nonce, hash_value, difficulty, job_id) {
                eprintln!("ERROR: Failed to write to hash log: {}", e);
            }
        }
    }

    pub fn flush() {
        if !LOGGING_ENABLED.load(Ordering::Relaxed) {
            return;
        }

        let instance = Self::get_instance();
        let file_guard = instance.file.lock().unwrap();
        if let Some(mut file) = file_guard.as_ref() {
            if let Err(e) = file.flush() {
                eprintln!("ERROR: Failed to flush hash log: {}", e);
            }
        }
    }
}

// Public functions for external use
pub fn init_hash_logger() {
    HashLogger::init();
}

pub fn log_hash_value(nonce: u32, hash_value: u64, difficulty: u64, job_id: &str) {
    HashLogger::log_hash(nonce, hash_value, difficulty, job_id);
}

pub fn flush_hash_log() {
    HashLogger::flush();
}
