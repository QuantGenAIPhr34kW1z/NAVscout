use anyhow::Result;
use std::path::Path;

pub fn check_spool(spool_dir: &str, max_mb: u64) -> Result<()> {
    let p = Path::new(spool_dir);
    if p.exists() {
        anyhow::ensure!(p.is_dir(), "uplink.spool_dir is not a dir: {}", spool_dir);
    }
    anyhow::ensure!(max_mb >= 8, "uplink.spool_max_mb too small; set >= 8MB");
    Ok(())
}
