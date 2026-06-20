use anyhow::{bail, Result};

pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub walrus_publisher_url: String,
    /// How many minutes before a receipt is eligible for Walrus archiving.
    pub archive_after_minutes: i64,
    /// Base URL of the coordinator, used to fetch the live peer registry
    /// (`GET /v1/nodes`) for node status/details.
    pub coordinator_url: String,
    /// Accept the coordinator's self-signed TLS cert (it presents an
    /// attested, not CA-issued, certificate).
    pub insecure_coordinator: bool,
    /// A node counts as "online" if its last_seen is within this many
    /// seconds. The node re-announces every 30s; default 90s (3x) avoids
    /// flapping while staying far tighter than the coordinator's own
    /// 600s registry eviction.
    pub node_online_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "".into());
        if database_url.is_empty() {
            bail!("DATABASE_URL is required");
        }
        Ok(Self {
            database_url,
            bind: std::env::var("INDEXER_BIND")
                .unwrap_or_else(|_| "0.0.0.0:3100".into()),
            walrus_publisher_url: std::env::var("WALRUS_PUBLISHER_URL")
                .unwrap_or_else(|_| "https://publisher.walrus.site".into()),
            archive_after_minutes: std::env::var("ARCHIVE_AFTER_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            coordinator_url: std::env::var("COORDINATOR_URL")
                .unwrap_or_else(|_| "https://127.0.0.1:4000".into()),
            insecure_coordinator: std::env::var("INSECURE_COORDINATOR")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            node_online_ttl_secs: std::env::var("NODE_ONLINE_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
        })
    }
}
