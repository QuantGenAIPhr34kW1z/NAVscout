use anyhow::{Context, Result};
use mavlink::{
    common::{
        MavMessage, HEARTBEAT_DATA, MavAutopilot, MavModeFlag, MavState,
        COMMAND_LONG_DATA, MavCmd, MavType, SYS_STATUS_DATA,
    },
    MavConnection, MavHeader,
};
use tokio_serial::SerialPortBuilderExt;
use tracing::{info, warn};

use crate::safety::CommandRateLimit;

#[derive(Debug, Clone, Default)]
pub struct BatteryStatus {
    pub voltage: Option<f32>,      // Volts
    pub current: Option<f32>,      // Amps
    pub remaining: Option<u8>,     // Percent 0-100
}

pub struct FcLink {
    conn: Box<dyn MavConnection<MavMessage> + Send>,
    hdr: MavHeader,
    target_sys: u8,
    target_comp: u8,
    seen_heartbeat: bool,
    limiter: CommandRateLimit,
    allow_rtl: bool,
    allow_hold: bool,
    require_heartbeat: bool,
    battery: BatteryStatus,
}

impl FcLink {
    pub fn open(
        dev: &str,
        baud: u32,
        sys_id: u8,
        comp_id: u8,
        target_sys: u8,
        target_comp: u8,
        allow_rtl: bool,
        allow_hold: bool,
        require_heartbeat: bool,
    ) -> Result<Self> {
        // quick validate device
        let _ = tokio_serial::new(dev, baud)
            .open_native_async()
            .with_context(|| format!("open fc serial device {}", dev))?;

        let url = format!("serial:{}:{}", dev, baud);
        let conn = mavlink::connect::<MavMessage>(&url)
            .with_context(|| format!("mavlink connect {}", url))?;

        Ok(Self {
            conn,
            hdr: MavHeader { system_id: sys_id, component_id: comp_id, sequence: 0 },
            target_sys,
            target_comp,
            seen_heartbeat: false,
            limiter: CommandRateLimit::new(std::time::Duration::from_secs(2)),
            allow_rtl,
            allow_hold,
            require_heartbeat,
            battery: BatteryStatus::default(),
        })
    }

    /// Best-effort: returns Ok(None) if recv fails.
    /// Some backends may block; for main loop use a dedicated reader thread/task.
    pub fn poll_once_nonblocking(&mut self) -> Result<Option<MavMessage>> {
        match self.conn.recv() {
            Ok((_hdr, msg)) => {
                if matches!(msg, MavMessage::HEARTBEAT(_)) {
                    self.seen_heartbeat = true;
                }
                // Update battery status from SYS_STATUS
                if let MavMessage::SYS_STATUS(status) = &msg {
                    self.update_battery(status);
                }
                Ok(Some(msg))
            }
            Err(_e) => Ok(None),
        }
    }

    fn update_battery(&mut self, status: &SYS_STATUS_DATA) {
        // voltage_battery is in millivolts, convert to volts
        if status.voltage_battery != u16::MAX {
            self.battery.voltage = Some(status.voltage_battery as f32 / 1000.0);
        }
        // current_battery is in centiamps (0.01A), convert to amps
        // -1 means invalid
        if status.current_battery != -1 {
            self.battery.current = Some(status.current_battery as f32 / 100.0);
        }
        // battery_remaining is percentage 0-100, -1 means invalid
        if status.battery_remaining >= 0 && status.battery_remaining <= 100 {
            self.battery.remaining = Some(status.battery_remaining as u8);
        }
    }

    pub fn battery_status(&self) -> &BatteryStatus {
        &self.battery
    }

    pub fn send_heartbeat(&mut self) -> Result<()> {
        let hb = HEARTBEAT_DATA {
            custom_mode: 0,
            mavtype: MavType::MAV_TYPE_ONBOARD_CONTROLLER,
            autopilot: MavAutopilot::MAV_AUTOPILOT_INVALID,
            base_mode: MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED,
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        };
        self.send(MavMessage::HEARTBEAT(hb))
    }

    pub fn cmd_rtl(&mut self) -> Result<()> {
        if !self.allow_rtl {
            anyhow::bail!("FC RTL command disabled by config");
        }
        if self.require_heartbeat && !self.seen_heartbeat {
            anyhow::bail!("refusing RTL: no heartbeat seen yet");
        }
        if !self.limiter.allow_rtl() {
            warn!("RTL rate-limited");
            return Ok(());
        }

        let cmd = COMMAND_LONG_DATA {
            target_system: self.target_sys,
            target_component: self.target_comp,
            command: MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH.into(),
            confirmation: 0,
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: 0.0,
            param5: 0.0,
            param6: 0.0,
            param7: 0.0,
        };
        info!("FC: sending RTL");
        self.send(MavMessage::COMMAND_LONG(cmd))
    }

    pub fn cmd_hold(&mut self) -> Result<()> {
        if !self.allow_hold {
            anyhow::bail!("FC HOLD command disabled by config");
        }
        if self.require_heartbeat && !self.seen_heartbeat {
            anyhow::bail!("refusing HOLD: no heartbeat seen yet");
        }
        if !self.limiter.allow_hold() {
            warn!("HOLD rate-limited");
            return Ok(());
        }

        let cmd = COMMAND_LONG_DATA {
            target_system: self.target_sys,
            target_component: self.target_comp,
            command: MavCmd::MAV_CMD_NAV_LOITER_UNLIM.into(),
            confirmation: 0,
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: 0.0,
            param5: 0.0,
            param6: 0.0,
            param7: 0.0,
        };
        info!("FC: sending HOLD/LOITER");
        self.send(MavMessage::COMMAND_LONG(cmd))
    }

    fn send(&mut self, msg: MavMessage) -> Result<()> {
        self.hdr.sequence = self.hdr.sequence.wrapping_add(1);
        self.conn.send(&self.hdr, &msg).context("mavlink send")?;
        Ok(())
    }
}
