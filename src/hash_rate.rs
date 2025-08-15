use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use lazy_static::lazy_static;
use once_cell::sync::Lazy;

#[derive(Clone)]
struct HashEvent {
    timestamp: Instant,
    count: u64,
}

// Static start time for the application
static START_TIME: Lazy<Instant> = Lazy::new(Instant::now);

// Global instance of HashRateTracker
lazy_static! {
    static ref HASH_RATE_TRACKER_INSTANCE: Arc<Mutex<HashRateTracker>> = {
        let tracker = HashRateTracker::new(); // Default to no debug
        Arc::new(Mutex::new(tracker))
    };
}

pub fn init_hash_rate_tracker(debug_all: bool) {
    let mut tracker = HASH_RATE_TRACKER_INSTANCE.lock().unwrap();
    tracker.debug_all = debug_all;
}

pub fn get_hash_rate_tracker() -> &'static Arc<Mutex<HashRateTracker>> {
    &HASH_RATE_TRACKER_INSTANCE
}

#[derive(Clone)]
pub struct HashRateTracker {
    hash_events: Arc<Mutex<VecDeque<HashEvent>>>,
    warmup_duration: Duration,
    window_duration: Duration,
    warmup_complete: Arc<AtomicBool>,
    debug_all: bool,
}

impl HashRateTracker {
    pub fn new() -> Self {
        Self {
            hash_events: Arc::new(Mutex::new(VecDeque::new())),
            warmup_duration: Duration::from_secs(45),
            window_duration: Duration::from_secs(120),
            warmup_complete: Arc::new(AtomicBool::new(false)),
            debug_all: false,
        }
    }

    #[inline(always)]
    pub fn increment(&self, count: u64) {
        let now = Instant::now();
        
        let global_elapsed = now.duration_since(*START_TIME);
        
        if global_elapsed < self.warmup_duration {
            if self.debug_all {
                eprintln!("DEBUG: Still in warmup - global_time: {:.2}s, needed: {:.2}s", 
                         global_elapsed.as_secs_f64(), self.warmup_duration.as_secs_f64());
            }
            return;
        }
        
        if !self.warmup_complete.load(Ordering::Relaxed) {
            self.warmup_complete.store(true, Ordering::SeqCst);
            if self.debug_all {
                eprintln!("DEBUG: Warmup completed at {:.2}s", global_elapsed.as_secs_f64());
            }
        }
        
        self.hash_events.lock().unwrap().push_back(HashEvent {
            timestamp: now,
            count,
        });
        
        let cutoff = now - self.window_duration;
        let mut events = self.hash_events.lock().unwrap();
        while let Some(event) = events.front() {
            if event.timestamp < cutoff {
                events.pop_front();
            } else {
                break;
            }
        }
    }

    #[inline(always)]
    pub fn get_total_hashes(&self) -> u64 {
        let now = Instant::now();
        
        let cutoff = now - self.window_duration;
        let mut events = self.hash_events.lock().unwrap();
        while let Some(event) = events.front() {
            if event.timestamp < cutoff {
                events.pop_front();
            } else {
                break;
            }
        }
        
        events.iter().map(|event| event.count).sum()
    }

    #[inline(always)]
    pub fn get_hash_rate(&self) -> f64 {
        let now = Instant::now();
        
        let cutoff = now - self.window_duration;
        let mut events = self.hash_events.lock().unwrap();
        while let Some(event) = events.front() {
            if event.timestamp < cutoff {
                events.pop_front();
            } else {
                break;
            }
        }
        
        let mut total_hashes = 0u64;
        let mut first_timestamp = None;
        
        for event in events.iter() {
            if event.timestamp >= cutoff {
                total_hashes += event.count;
                if first_timestamp.is_none() {
                    first_timestamp = Some(event.timestamp);
                }
            }
        }
        
        let Some(first_ts) = first_timestamp else {
            return 0.0;
        };
        
        let elapsed_duration = now - first_ts;
        let elapsed = elapsed_duration.as_secs_f64().max(0.001);
        
        total_hashes as f64 / elapsed
    }

    #[inline(always)]
    pub fn get_elapsed_time(&self) -> Duration {
        // Return total runtime since START_TIME
        Instant::now().duration_since(*START_TIME)
    }
}
