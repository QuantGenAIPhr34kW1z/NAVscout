pub mod mav;
pub mod autodetect;
pub mod safety;
pub mod state;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FcConfig {
    pub enable: bool,

    /// If true, scout will probe candidate serial ports/bauds and pick the first
    /// that yields MAVLink HEARTBEAT.
    pub autodetect: bool,

    /// When autodetect=false: fixed port config
    pub serial_dev: Option<String>,
    pub baud: Option<u32>,

    /// Autodetect candidates (paths). Example:
    /// ["/dev/serial0","/dev/ttyAMA0","/dev/ttyS0","/dev/ttyUSB0","/dev/ttyACM0"]
    pub candidate_devs: Option<Vec<String>>,

    /// Autodetect candidate baud rates (common ArduPilot telemetry values).
    pub candidate_bauds: Option<Vec<u32>>,

    /// Heartbeat wait per probe attempt
    pub heartbeat_timeout_ms: Option<u64>,

    /// MAVLink ids we use (Pi side)
    pub sys_id: u8,
    pub comp_id: u8,

    /// target system/component (FC side). 1/1 is common for ArduPilot.
    pub target_sys: u8,
    pub target_comp: u8,

    /// Hard safety: only allow these high-level commands
    pub allow_rtl: bool,
    pub allow_hold: bool,

    /// Require seeing FC heartbeat before sending commands
    pub require_heartbeat: bool,

    /// Optional: heartbeat send interval (companion heartbeat). Default 1s.
    pub send_heartbeat_hz: Option<f32>,
}
