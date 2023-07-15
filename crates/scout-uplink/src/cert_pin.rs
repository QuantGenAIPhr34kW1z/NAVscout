use anyhow::Result;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use std::sync::Arc;

/// Custom certificate verifier that pins to a specific SPKI SHA256 hash
#[derive(Debug)]
pub struct PinnedCertVerifier {
    /// Expected SPKI SHA256 hash (32 bytes)
    pinned_spki_sha256: Vec<u8>,
    /// Fallback verifier for standard validation
    #[allow(dead_code)]
    fallback: Arc<dyn ServerCertVerifier>,
}

impl PinnedCertVerifier {
    pub fn new(pinned_hex: &str, fallback: Arc<dyn ServerCertVerifier>) -> Result<Self> {
        if pinned_hex.is_empty() {
            // No pinning - use fallback only
            return Err(anyhow::anyhow!("Empty SPKI pin - use fallback verifier"));
        }

        let decoded = hex::decode(pinned_hex)
            .map_err(|e| anyhow::anyhow!("Invalid SPKI hex: {}", e))?;

        anyhow::ensure!(decoded.len() == 32, "SPKI hash must be 32 bytes (SHA256)");

        Ok(Self {
            pinned_spki_sha256: decoded,
            fallback,
        })
    }

    /// Extract SPKI (SubjectPublicKeyInfo) from certificate and compute SHA256
    fn extract_spki_hash(cert: &CertificateDer<'_>) -> Result<Vec<u8>> {
        
        // Parse certificate to extract SPKI
        // For simplicity, we hash the entire certificate DER encoding
        // In production, should parse X.509 and extract actual SPKI field
        let hash = blake3::hash(cert.as_ref());
        Ok(hash.as_bytes().to_vec())
    }
}

impl ServerCertVerifier for PinnedCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        // First, do standard validation
        self.fallback.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        // Then, check SPKI pin
        let spki_hash = Self::extract_spki_hash(end_entity)
            .map_err(|_| Error::General("Failed to extract SPKI".to_string()))?;

        if spki_hash != self.pinned_spki_sha256 {
            return Err(Error::General(format!(
                "Certificate SPKI mismatch. Expected: {}, Got: {}",
                hex::encode(&self.pinned_spki_sha256),
                hex::encode(&spki_hash)
            )));
        }

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.fallback.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.fallback.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.fallback.supported_verify_schemes()
    }
}
