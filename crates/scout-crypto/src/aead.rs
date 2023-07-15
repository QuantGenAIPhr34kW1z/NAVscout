use anyhow::Result;
use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
use rand::RngCore;

#[derive(Clone)]
pub struct AeadKey(pub [u8; 32]);

pub fn seal(key: &AeadKey, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&key.0).into());
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ct = cipher.encrypt(XNonce::from_slice(&nonce), chacha20poly1305::aead::Payload { msg: plaintext, aad })
        .map_err(|e| anyhow::anyhow!("AEAD encryption failed: {:?}", e))?;
    let mut out = Vec::with_capacity(24 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn open(key: &AeadKey, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    anyhow::ensure!(blob.len() >= 24, "ciphertext too short");
    let (nonce, ct) = blob.split_at(24);
    let cipher = XChaCha20Poly1305::new((&key.0).into());
    let pt = cipher.decrypt(XNonce::from_slice(nonce), chacha20poly1305::aead::Payload { msg: ct, aad })
        .map_err(|e| anyhow::anyhow!("AEAD decryption failed: {:?}", e))?;
    Ok(pt)
}
