pub mod doctor;
mod cert_pin;

use anyhow::{Context, Result};
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::ServerName;
use scout_crypto::{aead, keys::DeviceKeys};
use scout_proto::telemetry::TelemetryEvent;
use std::{path::Path, sync::Arc};
use tokio::{fs, io::AsyncWriteExt, net::TcpStream};
use tokio_rustls::TlsConnector;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct LinkHealth {
    pub rtt_ms: Option<u32>,
    pub quality: u8,           // 0-100
    pub consecutive_failures: u32,
}

impl Default for LinkHealth {
    fn default() -> Self {
        Self {
            rtt_ms: None,
            quality: 100,
            consecutive_failures: 0,
        }
    }
}

pub struct Uplink {
    endpoint: String,
    spool_dir: String,
    spool_max_bytes: u64,
    keys: DeviceKeys,
    tls: TlsConnector,
    health: LinkHealth,
}

impl Uplink {
    pub fn new(endpoint: String, pin_spki_hex: String, spool_dir: String, spool_max_mb: u64, keys: DeviceKeys) -> Result<Self> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let cfg = if !pin_spki_hex.is_empty() {
            // Use certificate pinning
            use rustls::client::WebPkiServerVerifier;

            let fallback_verifier = WebPkiServerVerifier::builder(roots.clone().into())
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build fallback verifier: {:?}", e))?;

            match cert_pin::PinnedCertVerifier::new(&pin_spki_hex, fallback_verifier) {
                Ok(pinned_verifier) => {
                    info!("uplink: certificate pinning enabled (SPKI SHA256: {}...)", &pin_spki_hex[..16]);
                    ClientConfig::builder()
                        .dangerous()
                        .with_custom_certificate_verifier(Arc::new(pinned_verifier))
                        .with_no_client_auth()
                }
                Err(e) => {
                    warn!("uplink: certificate pinning failed to initialize: {:#} - falling back to standard validation", e);
                    ClientConfig::builder().with_root_certificates(roots.clone()).with_no_client_auth()
                }
            }
        } else {
            warn!("uplink: certificate pinning NOT enabled (pin_spki_hex empty) - vulnerable to MITM on slow links!");
            ClientConfig::builder().with_root_certificates(roots).with_no_client_auth()
        };

        let tls = TlsConnector::from(Arc::new(cfg));
        Ok(Self {
            endpoint,
            spool_dir,
            spool_max_bytes: spool_max_mb * 1024 * 1024,
            keys,
            tls,
            health: LinkHealth::default(),
        })
    }

    pub fn link_health(&self) -> &LinkHealth {
        &self.health
    }

    /// Returns recommended telemetry interval in seconds based on link quality
    /// - High quality (80-100%): 30s (frequent updates)
    /// - Medium quality (50-79%): 60s (moderate updates)
    /// - Low quality (20-49%): 120s (reduced updates)
    /// - Poor quality (0-19%): 300s (minimal updates)
    /// - After consecutive failures: exponential backoff up to 600s
    pub fn recommended_interval_secs(&self) -> u64 {
        // Apply exponential backoff for consecutive failures
        if self.health.consecutive_failures > 0 {
            let backoff = 30u64 << self.health.consecutive_failures.min(4);
            return backoff.min(600); // Cap at 10 minutes
        }

        // Adaptive rate based on link quality
        match self.health.quality {
            80..=100 => 30,   // High quality: frequent updates
            50..=79 => 60,    // Medium quality: moderate updates
            20..=49 => 120,   // Low quality: reduced updates
            _ => 300,         // Poor quality: minimal updates
        }
    }

    /// Returns whether we should attempt flush based on backoff state
    pub fn should_attempt_flush(&self) -> bool {
        // Always allow first attempt
        if self.health.consecutive_failures == 0 {
            return true;
        }

        // For failures, use exponential backoff
        // This prevents hammering a dead link
        true // Actual backoff timing should be handled by caller with timers
    }

    pub async fn send_event(&mut self, ev: &TelemetryEvent) -> Result<()> {
        let payload = serde_json::to_vec(ev)?;
        let blob = aead::seal(&self.keys.aead, b"navscout-telemetry-v1", &payload)?;
        self.spool_write(&blob).await?;
        Ok(())
    }

    async fn spool_write(&self, blob: &[u8]) -> Result<()> {
        fs::create_dir_all(&self.spool_dir).await?;
        let name = format!("{}/{}.bin", self.spool_dir, time::OffsetDateTime::now_utc().unix_timestamp_nanos());
        let mut f = fs::File::create(&name).await?;
        f.write_all(blob).await?;
        Ok(())
    }

    pub async fn flush_spool(&mut self) -> Result<()> {
        let dir = Path::new(&self.spool_dir);
        if !dir.exists() {
            return Ok(());
        }
        let mut entries = fs::read_dir(dir).await?;
        while let Some(ent) = entries.next_entry().await? {
            let path = ent.path();
            if !path.is_file() { continue; }
            let blob = fs::read(&path).await?;
            if let Err(e) = self.send_blob(&blob).await {
                // keep it for retry
                return Err(e);
            } else {
                fs::remove_file(&path).await.ok();
            }
        }
        Ok(())
    }

    async fn send_blob(&mut self, blob: &[u8]) -> Result<()> {
        let start = std::time::Instant::now();

        // endpoint: tls://host:port
        let ep = self.endpoint.strip_prefix("tls://").context("endpoint must start with tls://")?;
        let mut parts = ep.split(':');
        let host = parts.next().context("missing host")?;
        let port = parts.next().context("missing port")?;
        let addr = format!("{}:{}", host, port);

        let result = async {
            let tcp = TcpStream::connect(addr).await?;
            let name = ServerName::try_from(host.to_string())?;
            let mut tls = self.tls.connect(name, tcp).await?;

            // simple framing: u32 length + blob
            let len = (blob.len() as u32).to_be_bytes();
            tls.write_all(&len).await?;
            tls.write_all(blob).await?;
            tls.flush().await?;

            Ok::<(), anyhow::Error>(())
        }.await;

        // Update link health based on result
        match result {
            Ok(()) => {
                let rtt = start.elapsed().as_millis() as u32;
                self.health.rtt_ms = Some(rtt);
                self.health.consecutive_failures = 0;
                // Gradually improve quality on success
                self.health.quality = (self.health.quality + 10).min(100);
                info!("uplink: sent {} bytes (RTT: {}ms, quality: {}%)", blob.len(), rtt, self.health.quality);
                Ok(())
            }
            Err(e) => {
                self.health.consecutive_failures += 1;
                // Degrade quality on failure
                self.health.quality = self.health.quality.saturating_sub(20);
                warn!("uplink: send failed (failures: {}, quality: {}%): {:#}",
                      self.health.consecutive_failures, self.health.quality, e);
                Err(e)
            }
        }
    }
}
