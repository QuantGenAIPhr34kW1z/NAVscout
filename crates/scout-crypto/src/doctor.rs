use anyhow::Result;
use std::path::Path;

use crate::keys::KeyConfig;

pub fn check_keys(cfg: &KeyConfig) -> Result<()> {
    let p = Path::new(&cfg.key_path);
    anyhow::ensure!(p.exists(), "crypto.key_path missing: {}", cfg.key_path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let md = std::fs::metadata(p)?;
        let mode = md.mode() & 0o777;
        anyhow::ensure!(mode == 0o600, "key permissions should be 0600, got {:o}", mode);
    }
    Ok(())
}
