use anyhow::{bail, Result};

pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub walrus_publisher_url: String,
    /// How many hours before a receipt is eligible for Walrus archiving.
    pub archive_after_hours: i64,
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
            archive_after_hours: std::env::var("ARCHIVE_AFTER_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
        })
    }
}
