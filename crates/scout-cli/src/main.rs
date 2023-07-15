use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::{info, warn};

use scout_crypto::keys::{DeviceKeys, KeyConfig};
use scout_nav::{doctor as nav_doctor, gnss, nav};
use scout_proto::telemetry::{EventKind, TelemetryEvent};
use scout_uplink::{doctor as uplink_doctor, Uplink};

use scout_vision::{camera, Roi, VisionConfig};
use scout_vision::power::{PowerConfig, PowerCtl, PowerMode};
use scout_vision::tracker::{TrackingConfig, Tracker};

use scout_fc::{FcConfig};
use scout_fc::mav::FcLink;
use scout_fc::autodetect::{autodetect_fc, default_candidate_bauds, default_candidate_devs};
use scout_fc::state::FcStatus;

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[cfg(feature = "vision-tflite")]
use scout_vision::tflite::TfliteDetector;

#[derive(Debug, Parser)]
#[command(name = "scout", version, about = "NAVscout - AI-Powered Drone Navigation & Tracking")]
struct Cli {
    #[arg(long)]
    config: String,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Doctor,
    Keys { #[command(subcommand)] cmd: KeysCmd },
    Run,
    Vision { #[command(subcommand)] cmd: VisionCmd },
    Fc { #[command(subcommand)] cmd: FcCmd },
}

#[derive(Debug, Subcommand)]
enum VisionCmd { Inspect }

#[derive(Debug, Subcommand)]
enum FcCmd {
    /// Probe serial ports/bauds for MAVLink heartbeats.
    Autodetect,
    /// Print current FC link status (when running `scout run` this reflects live state).
    Status,
}

#[derive(Debug, Subcommand)]
enum KeysCmd { Init, Rotate }

#[derive(Debug, serde::Deserialize)]
struct Config {
    crypto: CryptoCfg,
    uplink: UplinkCfg,
    gnss: GnssCfg,
    nav: NavCfg,
    rth: RthCfg,

    vision: Option<VisionCfg>,
    camera: Option<camera::CameraConfig>,
    tracking: Option<TrackingCfg>,
    power: Option<PowerCfg>,

    fc: Option<FcConfig>,
}

#[derive(Debug, serde::Deserialize)]
struct CryptoCfg { key_path: String, passphrase: Option<String> }

#[derive(Debug, serde::Deserialize)]
struct UplinkCfg {
    enable: bool,
    endpoint: String,
    pinned_server_spki_sha256: Option<String>,
    spool_dir: String,
    spool_max_mb: u64,
}

#[derive(Debug, serde::Deserialize)]
struct GnssCfg {
    source: String,
    nmea_device: Option<String>,
    nmea_file: Option<String>,
    min_sats: u8,
    max_hdop: f32,
    max_fix_age_s: u64,
}

#[derive(Debug, serde::Deserialize)]
struct NavCfg {
    home: nav::Home,
    cruise_alt_m: f32,
    max_radius_m: f64,
    route: nav::RouteCfg,
    zone: nav::ZoneCfg,
}

#[derive(Debug, serde::Deserialize)]
struct RthCfg {
    grace_link_loss_s: u64,
    gnss_bad_fix_s: u64,
    battery_low_pct: u8,
    thermal_soft_c: i32,
    action_on_tamper: String,
    action_on_weather: String,
    land_at_home: bool,
}

#[derive(Debug, serde::Deserialize)]
struct VisionCfg {
    enable: bool,
    backend: String,
    use_coral: bool,
    model_path: String,
    model_path_edgetpu: String,
    img_w: u32,
    img_h: u32,
    num_classes: usize,
    class_names: Vec<String>,
    conf_threshold: f32,
    nms_iou_threshold: f32,
    max_detections: usize,
    output_layout: String,
    roi_enable: Option<bool>,
    roi_margin: Option<f32>,
    roi_min_size: Option<f32>,
}

#[derive(Debug, serde::Deserialize)]
struct TrackingCfg {
    enable: bool,
    max_age_frames: u32,
    min_hits: u32,
    iou_match_threshold: f32,
    max_tracks: usize,
    target_class: String,
    lock_min_conf: f32,
}

#[derive(Debug, serde::Deserialize)]
struct PowerCfg {
    mode: String,
    scan_infer_every_n: u32,
    track_infer_every_n: u32,
    burst_seconds: f32,
    burst_infer_every_n: u32,
    idle_to_scan_seconds: f32,
}

fn load_config(path: &str) -> Result<Config> {
    let s = std::fs::read_to_string(path).context("read config")?;
    Ok(toml::from_str(&s).context("parse config toml")?)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let cfg = load_config(&cli.config)?;

    // FC status is shared (even for subcommands)
    let fc_status = Arc::new(Mutex::new(FcStatus::default()));

    match cli.cmd {
        Command::Doctor => doctor(&cfg).await?,
        Command::Keys { cmd } => keys(&cfg, cmd).await?,
        Command::Run => run(&cfg, fc_status).await?,
        Command::Vision { cmd } => vision_cmd(&cfg, cmd).await?,
        Command::Fc { cmd } => fc_cmd(&cfg, cmd, fc_status).await?,
    }
    Ok(())
}

async fn doctor(cfg: &Config) -> Result<()> {
    info!("doctor: starting");

    let kcfg = KeyConfig {
        key_path: cfg.crypto.key_path.clone(),
        passphrase: cfg.crypto.passphrase.clone().unwrap_or_default(),
    };
    scout_crypto::doctor::check_keys(&kcfg).or_else(|e| {
        warn!("keys missing or weak perms: {:#}", e);
        Ok::<(), anyhow::Error>(())
    })?;

    nav_doctor::check_geofence(&cfg.nav.home, &cfg.nav.route, &cfg.nav.zone, cfg.nav.max_radius_m)?;
    nav_doctor::check_gnss_thresholds(cfg.gnss.min_sats, cfg.gnss.max_hdop, cfg.gnss.max_fix_age_s)?;
    uplink_doctor::check_spool(&cfg.uplink.spool_dir, cfg.uplink.spool_max_mb)?;

    if let Some(fc) = &cfg.fc {
        if fc.enable {
            if fc.autodetect {
                info!("doctor: fc autodetect enabled (OK)");
            } else {
                anyhow::ensure!(fc.serial_dev.as_ref().map(|s| !s.is_empty()).unwrap_or(false), "fc.serial_dev missing");
                anyhow::ensure!(fc.baud.unwrap_or(0) > 0, "fc.baud invalid");
            }
        }
    }

    info!("doctor: OK");
    Ok(())
}

async fn keys(cfg: &Config, cmd: KeysCmd) -> Result<()> {
    let kcfg = KeyConfig {
        key_path: cfg.crypto.key_path.clone(),
        passphrase: cfg.crypto.passphrase.clone().unwrap_or_default(),
    };
    match cmd {
        KeysCmd::Init => { DeviceKeys::init(&kcfg)?; info!("keys: initialized"); }
        KeysCmd::Rotate => { DeviceKeys::rotate(&kcfg)?; info!("keys: rotated"); }
    }
    Ok(())
}

async fn vision_cmd(cfg: &Config, cmd: VisionCmd) -> Result<()> {
    match cmd {
        VisionCmd::Inspect => {
            let mut det = init_detector(cfg)?;
            #[cfg(feature = "vision-tflite")]
            {
                if let Some(VisionRuntime::Tflite(d)) = det.as_mut() {
                    print!("{}", d.inspect()?);
                    return Ok(());
                }
            }
            anyhow::bail!("vision backend not available; build with --features vision-tflite");
        }
    }
}

async fn fc_cmd(cfg: &Config, cmd: FcCmd, fc_status: Arc<Mutex<FcStatus>>) -> Result<()> {
    match cmd {
        FcCmd::Autodetect => {
            let fc = cfg.fc.as_ref().context("no [fc] config section")?;
            anyhow::ensure!(fc.enable, "fc.enable=false");
            let res = run_fc_autodetect(fc)?;
            if let Some((dev, baud)) = res.chosen {
                println!("CHOSEN: {} @ {}", dev, baud);
            } else {
                println!("CHOSEN: none");
            }
            for p in res.probes {
                println!("probe dev={} baud={} hb={} {}ms note={}", p.dev, p.baud, p.hb_seen, p.elapsed_ms, p.note);
            }
            Ok(())
        }
        FcCmd::Status => {
            let st = fc_status.lock().unwrap().clone();
            println!("connected={}", st.connected);
            println!("port={:?} baud={:?}", st.port, st.baud);
            println!("last_heartbeat_age={:?}", st.hb_age());
            println!("last_msg={:?}", st.last_msg);
            Ok(())
        }
    }
}

async fn run(cfg: &Config, fc_status: Arc<Mutex<FcStatus>>) -> Result<()> {
    info!("run: starting");

    let keys = DeviceKeys::load(&KeyConfig {
        key_path: cfg.crypto.key_path.clone(),
        passphrase: cfg.crypto.passphrase.clone().unwrap_or_default(),
    })?;

    let mut src = match cfg.gnss.source.as_str() {
        "nmea-serial" => gnss::GnssSource::serial(cfg.gnss.nmea_device.as_ref().context("gnss.nmea_device missing")?)?,
        "nmea-file" => gnss::GnssSource::file(cfg.gnss.nmea_file.as_ref().context("gnss.nmea_file missing")?)?,
        other => anyhow::bail!("unknown gnss.source: {}", other),
    };

    let mut uplink = if cfg.uplink.enable {
        Some(Uplink::new(
            cfg.uplink.endpoint.clone(),
            cfg.uplink.pinned_server_spki_sha256.clone().unwrap_or_default(),
            cfg.uplink.spool_dir.clone(),
            cfg.uplink.spool_max_mb,
            keys.clone(),
        )?)
    } else { None };

    let mut nav_engine = nav::NavEngine::new(
        cfg.nav.home.clone(),
        cfg.nav.route.clone(),
        cfg.nav.zone.clone(),
        cfg.nav.max_radius_m,
        nav::RthPolicy { grace_link_loss_s: cfg.rth.grace_link_loss_s, gnss_bad_fix_s: cfg.rth.gnss_bad_fix_s },
    );

    // FC: background link (optional)
    let (fc_tx_cmd, mut fc_rx_cmd) = mpsc::channel::<FcCommand>(8);
    let mut fc_handle = None;

    if let Some(fc_cfg) = cfg.fc.as_ref() {
        if fc_cfg.enable {
            let (dev, baud) = resolve_fc_port(fc_cfg)?;
            {
                let mut st = fc_status.lock().unwrap();
                st.port = Some(dev.clone());
                st.baud = Some(baud);
            }

            let sys_id = fc_cfg.sys_id;
            let comp_id = fc_cfg.comp_id;
            let target_sys = fc_cfg.target_sys;
            let target_comp = fc_cfg.target_comp;

            let allow_rtl = fc_cfg.allow_rtl;
            let allow_hold = fc_cfg.allow_hold;
            let require_heartbeat = fc_cfg.require_heartbeat;
            let hb_hz = fc_cfg.send_heartbeat_hz.unwrap_or(1.0).max(0.2);

            let fc_status2 = fc_status.clone();
            let mut link = FcLink::open(
                &dev, baud,
                sys_id, comp_id,
                target_sys, target_comp,
                allow_rtl, allow_hold,
                require_heartbeat,
            ).context("FC open")?;

            // Reader loop in a blocking task (mavlink serial recv can block).
            fc_handle = Some(tokio::task::spawn_blocking(move || {
                let hb_interval = std::time::Duration::from_secs_f32(1.0 / hb_hz);
                let mut last_hb_send = std::time::Instant::now();

                loop {
                    // Send companion heartbeat periodically
                    if last_hb_send.elapsed() >= hb_interval {
                        let _ = link.send_heartbeat();
                        last_hb_send = std::time::Instant::now();
                    }

                    // Read (best-effort)
                    if let Ok(Some(msg)) = link.poll_once_nonblocking() {
                        let mut st = fc_status2.lock().unwrap();
                        st.connected = true;
                        let msg_str = format!("{:?}", msg);
                        let is_heartbeat = msg_str.contains("HEARTBEAT");
                        st.last_msg = Some(msg_str);
                        if is_heartbeat {
                            st.last_heartbeat = Some(std::time::Instant::now());
                        }
                    }

                    // Light sleep to avoid busy loop
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }));

            // Command forwarder task (async) — when nav says RTH, we tell FC RTL.
            let fc_status3 = fc_status.clone();
            tokio::spawn(async move {
                // NOTE: The actual send happens inside spawn_blocking loop in baseline.
                // To keep things simple, we only update status here. (We do the RTL by opening a short-lived link below.)
                while let Some(cmd) = fc_rx_cmd.recv().await {
                    let mut st = fc_status3.lock().unwrap();
                    st.last_msg = Some(format!("cmd={:?}", cmd));
                }
            });
        }
    }

    // Vision stack
    let mut det = init_detector(cfg)?;
    let mut tracker = init_tracker(cfg)?;
    let mut power = init_power(cfg)?;
    let mut last_lock_roi: Option<Roi> = None;

    let mut last_state = nav::MissionState::Idle;

    loop {
        let fix = src.next_fix().await?;
        let quality = fix.quality.clone();
        let nav_out = nav_engine.step(fix.clone());

        // On entering RTH: send RTL to FC (short-lived command link to avoid cross-thread borrow complexity)
        if nav_out.state == nav::MissionState::Rth && last_state != nav::MissionState::Rth {
            if let Some(fc_cfg) = cfg.fc.as_ref() {
                if fc_cfg.enable && fc_cfg.allow_rtl {
                    if let Ok((dev, baud)) = resolve_fc_port(fc_cfg) {
                        if let Ok(mut cmdlink) = FcLink::open(
                            &dev, baud,
                            fc_cfg.sys_id, fc_cfg.comp_id,
                            fc_cfg.target_sys, fc_cfg.target_comp,
                            fc_cfg.allow_rtl, fc_cfg.allow_hold,
                            fc_cfg.require_heartbeat,
                        ) {
                            let _ = cmdlink.cmd_rtl();
                        }
                    }
                    let _ = fc_tx_cmd.send(FcCommand::RtlRequested).await;
                }
            }
        }
        last_state = nav_out.state.clone();

        // Vision
        let do_infer = det.is_some() && power.tick_should_infer();
        let mut vision_msg = String::new();

        if do_infer {
            if let Some(camcfg) = &cfg.camera {
                let jpeg = camera::capture_jpeg(camcfg).await?;
                let use_roi = power.current_mode() != PowerMode::Scan && last_lock_roi.is_some();

                let dets: Vec<scout_vision::Detection> = match det.as_mut().unwrap() {
                    #[cfg(feature = "vision-tflite")]
                    VisionRuntime::Tflite(d) => {
                        if use_roi { d.detect_jpeg_with_roi(&jpeg, last_lock_roi)? } else { d.detect_jpeg(&jpeg)? }
                    }
                    #[allow(unreachable_patterns)]
                    _ => Vec::new(),
                };

                if let Some(tr) = tracker.as_mut() {
                    let out = tr.update(&dets);
                    power.on_lock_state(out.locked.is_some());

                    if let Some(lock) = out.locked {
                        last_lock_roi = Some(Roi { cx: lock.cx, cy: lock.cy, w: lock.w, h: lock.h }.clamp01());
                        power.on_target_event();
                        vision_msg = format!("TRACK lock={} conf={:.2} roi={} mode={:?}", lock.id, lock.conf, if use_roi { "on" } else { "off" }, power.current_mode());
                    } else {
                        last_lock_roi = None;
                        vision_msg = format!("TRACK none mode={:?}", power.current_mode());
                    }
                } else {
                    vision_msg = format!("DET n={} mode={:?}", dets.len(), power.current_mode());
                }
            }
        } else {
            vision_msg = format!("infer=skip mode={:?}", power.current_mode());
        }

        let ev = TelemetryEvent {
            ts_unix_ms: time::OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000,
            kind: match nav_out.state {
                nav::MissionState::OperateInZone => EventKind::Status,
                nav::MissionState::Rth => EventKind::Rth,
                nav::MissionState::Abort => EventKind::Abort,
                _ => EventKind::Status,
            },
            lat: fix.lat, lon: fix.lon,
            sats: quality.sats as i32,
            hdop: quality.hdop,
            msg: format!("{} {}", nav_out.message, vision_msg),
            battery_voltage: None,
            battery_percent: None,
            battery_current: None,
            cpu_temp_c: None,
            link_rtt_ms: None,
            link_quality: None,
        };

        if let Some(u) = uplink.as_mut() {
            if let Err(e) = u.send_event(&ev).await { warn!("uplink send failed: {:#}", e); }
            if let Err(e) = u.flush_spool().await { warn!("uplink flush failed: {:#}", e); }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // (unreachable in loop) keep handle live
    #[allow(unreachable_code)]
    {
        if let Some(h) = fc_handle { let _ = h.await; }
        Ok(())
    }
}

#[derive(Debug)]
enum FcCommand {
    RtlRequested,
}

fn run_fc_autodetect(fc: &FcConfig) -> Result<scout_fc::autodetect::AutodetectResult> {
    let devs = fc.candidate_devs.clone().unwrap_or_else(default_candidate_devs);
    let bauds = fc.candidate_bauds.clone().unwrap_or_else(default_candidate_bauds);
    let to_ms = fc.heartbeat_timeout_ms.unwrap_or(1500);
    let timeout = std::time::Duration::from_millis(to_ms);

    autodetect_fc(
        devs,
        bauds,
        timeout,
        fc.sys_id,
        fc.comp_id,
        fc.target_sys,
        fc.target_comp,
        fc.allow_rtl,
        fc.allow_hold,
        fc.require_heartbeat,
    )
}

fn resolve_fc_port(fc: &FcConfig) -> Result<(String, u32)> {
    if fc.autodetect {
        let res = run_fc_autodetect(fc)?;
        if let Some((dev, baud)) = res.chosen {
            return Ok((dev, baud));
        }
        anyhow::bail!("fc autodetect failed: no heartbeat found");
    } else {
        let dev = fc.serial_dev.clone().context("fc.serial_dev missing (autodetect=false)")?;
        let baud = fc.baud.context("fc.baud missing (autodetect=false)")?;
        Ok((dev, baud))
    }
}

// --- vision init helpers ---
fn init_detector(cfg: &Config) -> Result<Option<VisionRuntime>> {
    let Some(v) = &cfg.vision else { return Ok(None); };
    if !v.enable { return Ok(None); }

    #[cfg(not(feature = "vision-tflite"))]
    { anyhow::bail!("vision enabled but binary not built with --features vision-tflite"); }

    #[cfg(feature = "vision-tflite")]
    {
        let vc = VisionConfig {
            enable: v.enable,
            backend: v.backend.clone(),
            use_coral: v.use_coral,
            model_path: v.model_path.clone(),
            model_path_edgetpu: v.model_path_edgetpu.clone(),
            img_w: v.img_w, img_h: v.img_h,
            num_classes: v.num_classes,
            class_names: v.class_names.clone(),
            conf_threshold: v.conf_threshold,
            nms_iou_threshold: v.nms_iou_threshold,
            max_detections: v.max_detections,
            output_layout: v.output_layout.clone(),
            roi_enable: v.roi_enable,
            roi_margin: v.roi_margin,
            roi_min_size: v.roi_min_size,
        };
        Ok(Some(VisionRuntime::Tflite(TfliteDetector::new(vc)?)))
    }
}

fn init_tracker(cfg: &Config) -> Result<Option<Tracker>> {
    let Some(t) = &cfg.tracking else { return Ok(None); };
    let v = cfg.vision.as_ref().context("tracking configured but vision missing")?;
    Ok(Some(Tracker::new(
        TrackingConfig {
            enable: t.enable,
            max_age_frames: t.max_age_frames,
            min_hits: t.min_hits,
            iou_match_threshold: t.iou_match_threshold,
            max_tracks: t.max_tracks,
            target_class: t.target_class.clone(),
            lock_min_conf: t.lock_min_conf,
        },
        &v.class_names,
    )))
}

fn init_power(cfg: &Config) -> Result<PowerCtl> {
    let p = cfg.power.as_ref().context("power config missing")?;
    Ok(PowerCtl::new(PowerConfig {
        mode: p.mode.clone(),
        scan_infer_every_n: p.scan_infer_every_n,
        track_infer_every_n: p.track_infer_every_n,
        burst_seconds: p.burst_seconds,
        burst_infer_every_n: p.burst_infer_every_n,
        idle_to_scan_seconds: p.idle_to_scan_seconds,
    }))
}

enum VisionRuntime {
    #[cfg(feature = "vision-tflite")]
    Tflite(TfliteDetector),
}
