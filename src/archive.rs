//! Walrus archiver. Packs unarchived routing receipts older than
//! ARCHIVE_AFTER_MINUTES into a CBOR batch, uploads to Walrus, and
//! stamps each row with the resulting blob_id.

use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::PgPool;

use crate::db;

pub struct WalrusClient {
    publisher_url: String,
    http: reqwest::Client,
}

impl WalrusClient {
    pub fn new(publisher_url: String) -> Self {
        Self {
            publisher_url,
            http: reqwest::Client::new(),
        }
    }

    /// Upload raw bytes to Walrus and return the blob_id.
    pub async fn upload(&self, data: Vec<u8>) -> Result<String> {
        let url = format!("{}/v1/blobs", self.publisher_url);
        let resp = self
            .http
            .put(&url)
            .header("Content-Type", "application/octet-stream")
            .body(data)
            .send()
            .await
            .context("walrus upload request")?
            .error_for_status()
            .context("walrus upload status")?;

        let body: serde_json::Value = resp.json().await.context("walrus upload body")?;
        let blob_id = body
            .get("newlyCreated")
            .or_else(|| body.get("alreadyCertified"))
            .and_then(|v| v.get("blobObject"))
            .and_then(|v| v.get("blobId"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .context("walrus response missing blobId")?;

        Ok(blob_id)
    }
}

/// Called by the cron job every 5 minutes.
pub async fn run_archive_job(pool: &PgPool, walrus: &Arc<WalrusClient>) -> Result<()> {
    // Read ARCHIVE_AFTER_MINUTES from env each tick so it's adjustable without restart.
    let minutes: i64 = std::env::var("ARCHIVE_AFTER_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let rows = db::unarchived_older_than(pool, minutes).await?;
    if rows.is_empty() {
        return Ok(());
    }

    tracing::info!(count = rows.len(), "archiving receipts to Walrus");

    // Pack into a CBOR array.
    let mut buf = Vec::new();
    let payloads: Vec<&serde_json::Value> = rows.iter().map(|r| &r.receipt_json).collect();
    ciborium::into_writer(&payloads, &mut buf).context("cbor encode")?;

    let blob_id = walrus.upload(buf).await?;
    tracing::info!(%blob_id, count = rows.len(), "walrus upload complete");

    // Stamp each row. Best-effort — a failure here means we'll re-archive
    // on the next tick, which is fine (Walrus deduplicates by content hash).
    for row in &rows {
        if let Err(e) = db::set_walrus_blob_id(pool, row.request_id, &blob_id).await {
            tracing::warn!(request_id = %row.request_id, err = %e, "failed to stamp blob_id");
        }
    }

    Ok(())
}
