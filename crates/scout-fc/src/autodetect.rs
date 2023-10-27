use anyhow::Result;
use tracing::{info, warn};
use std::time::{Duration, Instant};

use crate::mav::FcLink;

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub dev: String,
    pub baud: u32,
    pub hb_seen: bool,
    pub elapsed_ms: u64,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct AutodetectResult {
    pub chosen: Option<(String, u32)>,
    pub probes: Vec<ProbeResult>,
}

pub fn default_candidate_devs() -> Vec<String> {
    vec![
        "/dev/serial0".into(),
        "/dev/ttyAMA0".into(),
        "/dev/ttyS0".into(),
        "/dev/ttyUSB0".into(),
        "/dev/ttyUSB1".into(),
        "/dev/ttyACM0".into(),
        "/dev/ttyACM1".into(),
    ]
}

pub fn default_candidate_bauds() -> Vec<u32> {
    vec![57600, 115200, 230400, 921600]
}

pub fn autodetect_fc(
    candidate_devs: Vec<String>,
    candidate_bauds: Vec<u32>,
    heartbeat_timeout: Duration,
    sys_id: u8,
    comp_id: u8,
    target_sys: u8,
    target_comp: u8,
    allow_rtl: bool,
    allow_hold: bool,
    require_heartbeat: bool,
) -> Result<AutodetectResult> {
    let mut probes = Vec::new();

    for dev in candidate_devs {
        for baud in &candidate_bauds {
            let start = Instant::now();
            let mut note = String::new();
            let mut hb_seen = false;

            match FcLink::open(
                &dev, *baud,
                sys_id, comp_id,
                target_sys, target_comp,
                allow_rtl, allow_hold,
                require_heartbeat,
            ) {
                Ok(mut link) => {
                    // Wait briefly for heartbeat
                    while start.elapsed() < heartbeat_timeout {
                        if let Ok(Some(msg)) = link.poll_once_nonblocking() {
                            if msg.is_heartbeat() {
                                hb_seen = true;
                                note = "heartbeat".into();
                                break;
                            }
                        }
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    if hb_seen {
                        let elapsed_ms = start.elapsed().as_millis() as u64;
                        probes.push(ProbeResult {
                            dev: dev.clone(), baud: *baud, hb_seen: true, elapsed_ms,
                            note: note.clone(),
                        });
                        info!("fc autodetect: OK {} @ {}", dev, baud);
                        return Ok(AutodetectResult { chosen: Some((dev, *baud)), probes });
                    } else {
                        note = "no heartbeat".into();
                    }
                }
                Err(e) => {
                    note = format!("open/connect failed: {}", e);
                    warn!("fc autodetect probe failed dev={} baud={} err={}", dev, baud, e);
                }
            }

            probes.push(ProbeResult {
                dev: dev.clone(),
                baud: *baud,
                hb_seen,
                elapsed_ms: start.elapsed().as_millis() as u64,
                note,
            });
        }
    }

    Ok(AutodetectResult { chosen: None, probes })
}

// helper trait-ish on mavlink message without leaking mavlink type to callers
trait HeartbeatCheck {
    fn is_heartbeat(&self) -> bool;
}

impl HeartbeatCheck for mavlink::common::MavMessage {
    fn is_heartbeat(&self) -> bool {
        matches!(self, mavlink::common::MavMessage::HEARTBEAT(_))
    }
}
