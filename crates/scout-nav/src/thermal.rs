use anyhow::{Context, Result};
use std::path::Path;

/// Reads CPU temperature from Raspberry Pi 5 thermal zone
/// Returns temperature in degrees Celsius
pub fn read_cpu_temp() -> Result<f32> {
    #[cfg(target_os = "linux")]
    {
        // RPI5 thermal zone path
        let thermal_path = "/sys/class/thermal/thermal_zone0/temp";

        if !Path::new(thermal_path).exists() {
            // Fallback: try hwmon interface (some systems)
            let hwmon_path = "/sys/class/hwmon/hwmon0/temp1_input";
            if Path::new(hwmon_path).exists() {
                return read_millidegrees(hwmon_path);
            }
            anyhow::bail!("No thermal sensor found");
        }

        read_millidegrees(thermal_path)
    }

    #[cfg(not(target_os = "linux"))]
    {
        anyhow::bail!("Thermal monitoring only supported on Linux")
    }
}

#[cfg(target_os = "linux")]
fn read_millidegrees(path: &str) -> Result<f32> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read thermal sensor {}", path))?;

    let millidegrees: i32 = content.trim()
        .parse()
        .context("parse temperature value")?;

    // Convert from millidegrees to degrees Celsius
    Ok(millidegrees as f32 / 1000.0)
}

/// Thermal monitoring with throttling detection
pub struct ThermalMonitor {
    warn_temp_c: f32,
    critical_temp_c: f32,
}

impl ThermalMonitor {
    pub fn new(warn_temp_c: f32, critical_temp_c: f32) -> Self {
        Self {
            warn_temp_c,
            critical_temp_c,
        }
    }

    /// Default thresholds for Raspberry Pi 5
    /// - Warning: 70°C
    /// - Critical: 80°C (thermal throttling starts at 85°C on RPI5)
    pub fn default() -> Self {
        Self::new(70.0, 80.0)
    }

    pub fn check(&self) -> Result<ThermalStatus> {
        let temp = read_cpu_temp()?;

        let status = if temp >= self.critical_temp_c {
            ThermalLevel::Critical
        } else if temp >= self.warn_temp_c {
            ThermalLevel::Warning
        } else {
            ThermalLevel::Normal
        };

        Ok(ThermalStatus { temp_c: temp, level: status })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct ThermalStatus {
    pub temp_c: f32,
    pub level: ThermalLevel,
}
