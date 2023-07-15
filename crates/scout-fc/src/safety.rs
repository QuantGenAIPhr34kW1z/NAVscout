use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CommandRateLimit {
    last_rtl: Option<Instant>,
    last_hold: Option<Instant>,
    min_interval: Duration,
}

impl CommandRateLimit {
    pub fn new(min_interval: Duration) -> Self {
        Self { last_rtl: None, last_hold: None, min_interval }
    }

    pub fn allow_rtl(&mut self) -> bool {
        let now = Instant::now();
        if let Some(t) = self.last_rtl {
            if now.duration_since(t) < self.min_interval { return false; }
        }
        self.last_rtl = Some(now);
        true
    }

    pub fn allow_hold(&mut self) -> bool {
        let now = Instant::now();
        if let Some(t) = self.last_hold {
            if now.duration_since(t) < self.min_interval { return false; }
        }
        self.last_hold = Some(now);
        true
    }
}
