use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::debug;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CameraConfig {
    pub mode: String,   // "libcamera-jpeg" | "v4l2-mjpeg"
    pub device: String, // /dev/video0 (v4l2)
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Pragmatic capture:
/// - libcamera-jpeg: call `libcamera-still -n -t 1 --width ... --height ... -o -`
///   returns a JPEG frame on stdout (simple, robust on Pi)
/// - v4l2-mjpeg: call `ffmpeg` to grab a single MJPEG frame (keeps Rust dependencies small)
pub async fn capture_jpeg(cfg: &CameraConfig) -> Result<Vec<u8>> {
    match cfg.mode.as_str() {
        "libcamera-jpeg" => capture_libcamera(cfg).await,
        "v4l2-mjpeg" => capture_v4l2_ffmpeg(cfg).await,
        other => anyhow::bail!("unknown camera.mode: {}", other),
    }
}

async fn capture_libcamera(cfg: &CameraConfig) -> Result<Vec<u8>> {
    let mut cmd = Command::new("libcamera-still");
    cmd.args([
        "-n",                 // no preview
        "-t", "1",            // 1ms
        "--width", &cfg.width.to_string(),
        "--height", &cfg.height.to_string(),
        "-o", "-",            // stdout
    ]);

    debug!("capture: libcamera-still");
    let out = cmd.output().await.context("run libcamera-still")?;
    anyhow::ensure!(out.status.success(), "libcamera-still failed");
    Ok(out.stdout)
}

async fn capture_v4l2_ffmpeg(cfg: &CameraConfig) -> Result<Vec<u8>> {
    // ffmpeg -f video4linux2 -input_format mjpeg -video_size WxH -i /dev/video0 -vframes 1 -f image2pipe -vcodec mjpeg -
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-hide_banner","-loglevel","error",
        "-f","video4linux2",
        "-input_format","mjpeg",
        "-video_size",&format!("{}x{}", cfg.width, cfg.height),
        "-i",&cfg.device,
        "-vframes","1",
        "-f","image2pipe",
        "-vcodec","mjpeg",
        "-",
    ]);

    debug!("capture: ffmpeg v4l2");
    let out = cmd.output().await.context("run ffmpeg capture")?;
    anyhow::ensure!(out.status.success(), "ffmpeg capture failed");
    Ok(out.stdout)
}
