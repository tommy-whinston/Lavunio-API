use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Login,
    Register,
    Reset,
}

impl Bucket {
    fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Register => "register",
            Self::Reset => "reset",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    started_at: Instant,
    count: u32,
}

pub struct RateLimiter {
    enabled: bool,
    window: Duration,
    login_limit: u32,
    register_limit: u32,
    reset_limit: u32,
    entries: Mutex<HashMap<String, Entry>>,
}

impl RateLimiter {
    pub fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.rate_limit_enabled,
            window: Duration::from_secs(config.rate_limit_window_secs.max(1)),
            login_limit: config.rate_limit_login,
            register_limit: config.rate_limit_register,
            reset_limit: config.rate_limit_reset,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, bucket: Bucket, key: &str) -> Result<(), ApiError> {
        if !self.enabled {
            return Ok(());
        }
        let limit = match bucket {
            Bucket::Login => self.login_limit,
            Bucket::Register => self.register_limit,
            Bucket::Reset => self.reset_limit,
        };
        if limit == 0 {
            return Ok(());
        }

        let key = format!("{}:{}", bucket.as_str(), key.trim());
        let now = Instant::now();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if entries.len() > 10_000 {
            entries
                .retain(|_, entry| now.saturating_duration_since(entry.started_at) < self.window);
        }

        let entry = entries.entry(key).or_insert(Entry {
            started_at: now,
            count: 0,
        });
        if now.saturating_duration_since(entry.started_at) >= self.window {
            entry.started_at = now;
            entry.count = 0;
        }

        if entry.count >= limit {
            let retry_after = self
                .window
                .saturating_sub(now.saturating_duration_since(entry.started_at))
                .as_secs()
                .max(1);
            return Err(ApiError::TooManyRequests(format!(
                "Слишком много попыток. Повторите через {retry_after} сек."
            )));
        }

        entry.count = entry.count.saturating_add(1);
        Ok(())
    }
}
