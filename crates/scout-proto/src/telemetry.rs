use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Status,
    Rth,
    Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub ts_unix_ms: i64,
    pub kind: EventKind,
    pub lat: f64,
    pub lon: f64,
    pub sats: i32,
    pub hdop: f32,
    pub msg: String,
    // Battery monitoring
    pub battery_voltage: Option<f32>,
    pub battery_percent: Option<u8>,
    pub battery_current: Option<f32>,
    // Thermal monitoring
    pub cpu_temp_c: Option<f32>,
    // Link health
    pub link_rtt_ms: Option<u32>,
    pub link_quality: Option<u8>,
}
