#[derive(Debug, Clone)]
pub struct PowerConfig {
    pub mode: String,              // scan | track | burst
    pub scan_infer_every_n: u32,
    pub track_infer_every_n: u32,
    pub burst_seconds: f32,
    pub burst_infer_every_n: u32,
    pub idle_to_scan_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMode {
    Scan,
    Track,
    Burst,
}

#[derive(Debug, Clone)]
pub struct PowerCtl {
    cfg: PowerConfig,
    mode: PowerMode,
    frame_counter: u64,
    burst_until: Option<std::time::Instant>,
    last_activity: std::time::Instant,
}

impl PowerCtl {
    pub fn new(cfg: PowerConfig) -> Self {
        let mode = match cfg.mode.as_str() {
            "scan" => PowerMode::Scan,
            "track" => PowerMode::Track,
            "burst" => PowerMode::Burst,
            _ => PowerMode::Scan,
        };
        let now = std::time::Instant::now();
        Self { cfg, mode, frame_counter: 0, burst_until: None, last_activity: now }
    }

    pub fn on_target_event(&mut self) {
        self.last_activity = std::time::Instant::now();
        self.burst_until = Some(self.last_activity + std::time::Duration::from_secs_f32(self.cfg.burst_seconds));
        self.mode = PowerMode::Burst;
    }

    pub fn on_lock_state(&mut self, has_lock: bool) {
        if has_lock {
            self.last_activity = std::time::Instant::now();
            if self.mode != PowerMode::Burst {
                self.mode = PowerMode::Track;
            }
        }
    }

    pub fn tick_should_infer(&mut self) -> bool {
        self.frame_counter += 1;
        self.refresh_mode();
        let n = match self.mode {
            PowerMode::Scan => self.cfg.scan_infer_every_n.max(1),
            PowerMode::Track => self.cfg.track_infer_every_n.max(1),
            PowerMode::Burst => self.cfg.burst_infer_every_n.max(1),
        };
        (self.frame_counter % n as u64) == 0
    }

    pub fn current_mode(&self) -> PowerMode { self.mode }

    fn refresh_mode(&mut self) {
        let now = std::time::Instant::now();

        // End burst if time elapsed
        if let Some(t) = self.burst_until {
            if now >= t {
                self.burst_until = None;
                self.mode = PowerMode::Track;
            } else {
                self.mode = PowerMode::Burst;
                return;
            }
        }

        // Idle fallback to scan
        let idle = now.duration_since(self.last_activity).as_secs_f32();
        if idle >= self.cfg.idle_to_scan_seconds {
            self.mode = PowerMode::Scan;
        }
    }
}
