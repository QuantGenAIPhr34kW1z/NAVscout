use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::fs::File;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use time::OffsetDateTime;
use std::sync::Mutex;
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct FixQuality {
    pub sats: u8,
    pub hdop: f32,
    pub fix_age_s: u64,
}

#[derive(Debug, Clone)]
pub struct GnssFix {
    pub lat: f64,
    pub lon: f64,
    pub quality: FixQuality,
    pub ts: OffsetDateTime,
}

pub enum GnssSource {
    Serial(BufReader<SerialStream>),
    File(BufReader<File>),
}

impl GnssSource {
    pub fn serial(dev: &str) -> Result<Self> {
        let port = tokio_serial::new(dev, 115200).open_native_async()
            .with_context(|| format!("open serial {}", dev))?;
        Ok(Self::Serial(BufReader::new(port)))
    }

    pub fn file(path: &str) -> Result<Self> {
        let f = std::fs::File::open(path).with_context(|| format!("open nmea file {}", path))?;
        let f = File::from_std(f);
        Ok(Self::File(BufReader::new(f)))
    }

    pub async fn next_fix(&mut self) -> Result<GnssFix> {
        let mut line = String::new();
        loop {
            line.clear();
            match self {
                GnssSource::Serial(r) => { r.read_line(&mut line).await?; }
                GnssSource::File(r) => {
                    let n = r.read_line(&mut line).await?;
                    if n == 0 {
                        // EOF: loop
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                }
            }
            if let Some(fix) = parse_nmea_line(line.trim())? {
                return Ok(fix);
            }
        }
    }
}

// Minimal NMEA parsing:
// - GGA: satellites + hdop
// - RMC: lat/lon
// Thread-safe storage for last GGA data
static LAST_GGA: Lazy<Mutex<Option<(u8, f32, OffsetDateTime)>>> =
    Lazy::new(|| Mutex::new(None));

fn parse_nmea_line(s: &str) -> Result<Option<GnssFix>> {
    if s.starts_with("$GNGGA") || s.starts_with("$GPGGA") {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > 9 {
            let sats: u8 = parts[7].parse().unwrap_or(0);
            let hdop: f32 = parts[8].parse().unwrap_or(99.9);
            *LAST_GGA.lock().unwrap() = Some((sats, hdop, OffsetDateTime::now_utc()));
        }
        return Ok(None);
    }

    if s.starts_with("$GNRMC") || s.starts_with("$GPRMC") {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > 6 {
            // parts[3]=lat ddmm.mmmm, parts[4]=N/S, parts[5]=lon dddmm.mmmm, parts[6]=E/W
            let lat = parse_deg_min(parts[3], parts[4]).unwrap_or(0.0);
            let lon = parse_deg_min(parts[5], parts[6]).unwrap_or(0.0);
            let now = OffsetDateTime::now_utc();

            let (sats, hdop, gga_ts) = LAST_GGA.lock().unwrap()
                .unwrap_or((0, 99.9, now));
            let fix_age_s = (now - gga_ts).whole_seconds().max(0) as u64;

            return Ok(Some(GnssFix {
                lat,
                lon,
                quality: FixQuality { sats, hdop, fix_age_s },
                ts: now,
            }));
        }
    }

    Ok(None)
}

fn parse_deg_min(v: &str, hemi: &str) -> Option<f64> {
    if v.is_empty() { return None; }
    // lat: ddmm.mmmm, lon: dddmm.mmmm
    let dot = v.find('.')?;
    let deg_len = if dot > 4 { 3 } else { 2 };
    let deg: f64 = v[..deg_len].parse().ok()?;
    let min: f64 = v[deg_len..].parse().ok()?;
    let mut out = deg + (min / 60.0);
    if hemi == "S" || hemi == "W" { out = -out; }
    Some(out)
}
