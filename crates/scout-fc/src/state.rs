use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct FcStatus {
    pub connected: bool,
    pub port: Option<String>,
    pub baud: Option<u32>,
    pub last_heartbeat: Option<Instant>,
    pub last_msg: Option<String>,
}

impl Default for FcStatus {
    fn default() -> Self {
        Self {
            connected: false,
            port: None,
            baud: None,
            last_heartbeat: None,
            last_msg: None,
        }
    }
}

impl FcStatus {
    pub fn hb_age(&self) -> Option<Duration> {
        self.last_heartbeat.map(|t| t.elapsed())
    }
}
