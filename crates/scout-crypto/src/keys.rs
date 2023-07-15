use anyhow::{Context, Result};
use argon2::{Argon2, password_hash::{SaltString, PasswordHash, PasswordVerifier}};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::aead::AeadKey;

#[derive(Clone)]
pub struct DeviceKeys {
    pub aead: AeadKey,
}

#[derive(Clone)]
pub struct KeyConfig {
    pub key_path: String,
    pub passphrase: String, // optional, empty means raw key file
}

impl DeviceKeys {
    pub fn init(cfg: &KeyConfig) -> Result<()> {
        let path = Path::new(&cfg.key_path);
        if let Some(p) = path.parent() { fs::create_dir_all(p)?; }
        anyhow::ensure!(!path.exists(), "key already exists");

        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);

        if cfg.passphrase.is_empty() {
            fs::write(path, &key)?;
        } else {
            // Wrap key: use proper Argon2 KDF to derive wrapping key from passphrase
            let salt = SaltString::generate(&mut rand::thread_rng());

            // Derive wrapping key using Argon2
            let mut wrapping_key = [0u8; 32];
            let argon = Argon2::default();
            argon.hash_password_into(
                cfg.passphrase.as_bytes(),
                salt.as_str().as_bytes(),
                &mut wrapping_key
            ).map_err(|e| anyhow::anyhow!("Argon2 KDF failed: {:?}", e))?;

            // Encrypt the master key with the derived wrapping key
            let wrapped = crate::aead::seal(&AeadKey(wrapping_key), b"navscout-keywrap", &key)?;

            // Store format: NAVSCOUT_KEYWRAP_V2\nsalt\nwrapped_blob
            let mut file = String::new();
            file.push_str("NAVSCOUT_KEYWRAP_V2\n");
            file.push_str(salt.as_str());
            file.push('\n');

            let mut f = fs::File::create(path)?;
            f.write_all(file.as_bytes())?;
            f.write_all(&wrapped)?;
            f.flush()?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    pub fn rotate(cfg: &KeyConfig) -> Result<()> {
        // rotate by re-init to a new file with ".new" then replace atomically
        let path = Path::new(&cfg.key_path);
        anyhow::ensure!(path.exists(), "key does not exist");
        let tmp = path.with_extension("new");
        let old = fs::read(path)?;
        let _ = old; // placeholder for future migration
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        fs::write(&tmp, &key)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn load(cfg: &KeyConfig) -> Result<DeviceKeys> {
        let path = Path::new(&cfg.key_path);
        let bytes = fs::read(path).context("read key file")?;

        if bytes.starts_with(b"NAVSCOUT_KEYWRAP_V2\n") {
            anyhow::ensure!(!cfg.passphrase.is_empty(), "passphrase required for wrapped key");
            // Parse header: magic + salt + wrapped blob
            let mut parts = bytes.splitn(3, |b| *b == b'\n');
            let _magic = parts.next().unwrap();
            let salt_line = parts.next().context("bad key header")?;
            let wrapped = parts.next().context("missing wrapped blob")?;

            let salt_str = std::str::from_utf8(salt_line)?;
            let salt = SaltString::from_b64(salt_str)
                .map_err(|e| anyhow::anyhow!("Invalid salt: {:?}", e))?;

            // Derive wrapping key using same Argon2 KDF
            let mut wrapping_key = [0u8; 32];
            let argon = Argon2::default();
            argon.hash_password_into(
                cfg.passphrase.as_bytes(),
                salt.as_str().as_bytes(),
                &mut wrapping_key
            ).map_err(|e| anyhow::anyhow!("Argon2 KDF failed: {:?}", e))?;

            // Decrypt master key
            let key = crate::aead::open(&AeadKey(wrapping_key), b"navscout-keywrap", wrapped)?;
            anyhow::ensure!(key.len() == 32, "bad key length");
            let mut k = [0u8; 32];
            k.copy_from_slice(&key);
            Ok(DeviceKeys { aead: AeadKey(k) })
        } else if bytes.starts_with(b"NAVSCOUT_KEYWRAP_V1\n") {
            // Legacy format support (will be migrated on next rotation)
            anyhow::ensure!(!cfg.passphrase.is_empty(), "passphrase required for wrapped key");
            let mut parts = bytes.splitn(3, |b| *b == b'\n');
            let _magic = parts.next().unwrap();
            let hash_line = parts.next().context("bad key header")?;
            let wrapped = parts.next().context("missing wrapped blob")?;

            let hash_str = std::str::from_utf8(hash_line)?;
            let parsed = PasswordHash::new(hash_str)
                .map_err(|e| anyhow::anyhow!("Invalid password hash: {:?}", e))?;
            Argon2::default().verify_password(cfg.passphrase.as_bytes(), &parsed)
                .map_err(|e| anyhow::anyhow!("Passphrase verification failed: {:?}", e))?;

            let wrapping = blake3::hash(hash_str.as_bytes()).as_bytes()[..32].try_into().unwrap();
            let key = crate::aead::open(&AeadKey(wrapping), b"navscout-keywrap", wrapped)?;
            anyhow::ensure!(key.len() == 32, "bad key length");
            let mut k = [0u8; 32];
            k.copy_from_slice(&key);
            Ok(DeviceKeys { aead: AeadKey(k) })
        } else {
            anyhow::ensure!(bytes.len() == 32, "raw key file must be 32 bytes");
            let mut k = [0u8; 32];
            k.copy_from_slice(&bytes);
            Ok(DeviceKeys { aead: AeadKey(k) })
        }
    }
}
