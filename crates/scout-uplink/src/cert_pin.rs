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
    fallback: Arc<dyn ServerCertVerifier>,
}

impl PinnedCertVerifier {
    pub fn new(pinned_hex: &str, fallback: Arc<dyn ServerCertVerifier>) -> Result<Self> {
        if pinned_hex.is_empty() {
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

    /// Extract SPKI (SubjectPublicKeyInfo) from X.509 certificate and compute SHA256
    ///
    /// X.509 structure (simplified):
    /// SEQUENCE {
    ///   tbsCertificate SEQUENCE {
    ///     version [0] EXPLICIT INTEGER OPTIONAL,
    ///     serialNumber INTEGER,
    ///     signature AlgorithmIdentifier,
    ///     issuer Name,
    ///     validity Validity,
    ///     subject Name,
    ///     subjectPublicKeyInfo SEQUENCE { ... },  <-- This is what we want
    ///     ...
    ///   },
    ///   signatureAlgorithm AlgorithmIdentifier,
    ///   signatureValue BIT STRING
    /// }
    fn extract_spki_hash(cert: &CertificateDer<'_>) -> Result<Vec<u8>> {
        let der = cert.as_ref();

        // Basic DER parsing to find SPKI
        // Skip outer SEQUENCE tag and length
        let (_, rest) = Self::parse_der_header(der)?;

        // Skip tbsCertificate SEQUENCE header
        let (tbs_len, tbs_content) = Self::parse_der_header(rest)?;
        let tbs = &tbs_content[..tbs_len];

        // Parse tbsCertificate fields to find SPKI
        let mut pos = 0;

        // Skip version [0] if present (context tag 0xA0)
        if tbs.get(pos) == Some(&0xA0) {
            let (field_len, _) = Self::parse_der_header(&tbs[pos..])?;
            pos += Self::der_header_len(&tbs[pos..])? + field_len;
        }

        // Skip serialNumber INTEGER
        let (field_len, _) = Self::parse_der_header(&tbs[pos..])?;
        pos += Self::der_header_len(&tbs[pos..])? + field_len;

        // Skip signature AlgorithmIdentifier SEQUENCE
        let (field_len, _) = Self::parse_der_header(&tbs[pos..])?;
        pos += Self::der_header_len(&tbs[pos..])? + field_len;

        // Skip issuer Name SEQUENCE
        let (field_len, _) = Self::parse_der_header(&tbs[pos..])?;
        pos += Self::der_header_len(&tbs[pos..])? + field_len;

        // Skip validity SEQUENCE
        let (field_len, _) = Self::parse_der_header(&tbs[pos..])?;
        pos += Self::der_header_len(&tbs[pos..])? + field_len;

        // Skip subject Name SEQUENCE
        let (field_len, _) = Self::parse_der_header(&tbs[pos..])?;
        pos += Self::der_header_len(&tbs[pos..])? + field_len;

        // Now we're at subjectPublicKeyInfo SEQUENCE
        let header_len = Self::der_header_len(&tbs[pos..])?;
        let (spki_len, _) = Self::parse_der_header(&tbs[pos..])?;
        let spki = &tbs[pos..pos + header_len + spki_len];

        // Hash SPKI with SHA256 (standard for certificate pinning)
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(spki);
        Ok(hasher.finalize().to_vec())
    }

    fn parse_der_header(data: &[u8]) -> Result<(usize, &[u8])> {
        anyhow::ensure!(data.len() >= 2, "DER too short");
        let _tag = data[0];
        let len_byte = data[1];

        if len_byte < 0x80 {
            // Short form: length is directly in second byte
            Ok((len_byte as usize, &data[2..]))
        } else {
            // Long form: len_byte & 0x7f gives number of length bytes
            let num_len_bytes = (len_byte & 0x7f) as usize;
            anyhow::ensure!(data.len() >= 2 + num_len_bytes, "DER length truncated");
            let mut len = 0usize;
            for i in 0..num_len_bytes {
                len = (len << 8) | (data[2 + i] as usize);
            }
            Ok((len, &data[2 + num_len_bytes..]))
        }
    }

    fn der_header_len(data: &[u8]) -> Result<usize> {
        anyhow::ensure!(data.len() >= 2, "DER too short");
        let len_byte = data[1];
        if len_byte < 0x80 {
            Ok(2)
        } else {
            Ok(2 + (len_byte & 0x7f) as usize)
        }
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
