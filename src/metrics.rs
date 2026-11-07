use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct Metrics {
    started_at: Instant,
    requests: AtomicU64,
    errors: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            requests: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn record(&self, status: u16) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        if status >= 400 {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn requests(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
