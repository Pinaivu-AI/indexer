//! Client for the coordinator's live peer registry (`GET /v1/nodes`).
//!
//! The indexer's own Postgres only holds *historical* data (routing
//! receipts). Whether a node is online right now, and its currently
//! advertised capabilities, live only in the coordinator's in-memory
//! peer registry — so node status/details are fetched from there.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One node as returned by the coordinator's `GET /v1/nodes`.
#[derive(Debug, Clone, Deserialize)]
pub struct CoordinatorNode {
    pub peer_id: String,
    pub models: Vec<String>,
    pub max_concurrent_jobs: u32,
    pub multiaddrs: Vec<String>,
    pub last_seen_ms: u64,
}

/// A node enriched with derived live status for API responses.
#[derive(Debug, Clone, Serialize)]
pub struct LiveNode {
    pub peer_id: String,
    pub online: bool,
    pub last_seen_ms: u64,
    pub seconds_since_seen: u64,
    pub models: Vec<String>,
    pub max_concurrent_jobs: u32,
    pub multiaddrs: Vec<String>,
}

#[derive(Clone)]
pub struct CoordinatorClient {
    base_url: String,
    http: reqwest::Client,
    online_ttl_secs: u64,
}

impl CoordinatorClient {
    pub fn new(base_url: String, insecure: bool, online_ttl_secs: u64) -> Self {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(insecure)
            .build()
            .expect("build reqwest client");
        Self {
            base_url,
            http,
            online_ttl_secs,
        }
    }

    /// Fetch the live peer registry and derive online status for each node.
    pub async fn live_nodes(&self) -> Result<Vec<LiveNode>> {
        let url = format!("{}/v1/nodes", self.base_url.trim_end_matches('/'));
        let nodes: Vec<CoordinatorNode> = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?
            .error_for_status()
            .context("coordinator /v1/nodes status")?
            .json()
            .await
            .context("decode /v1/nodes")?;

        let now_ms = now_ms();
        Ok(nodes.into_iter().map(|n| self.to_live(n, now_ms)).collect())
    }

    /// Fetch the live status of a single node, if it's currently in the
    /// registry. Returns None if the coordinator doesn't know this peer.
    pub async fn live_node(&self, peer_id: &str) -> Result<Option<LiveNode>> {
        Ok(self
            .live_nodes()
            .await?
            .into_iter()
            .find(|n| n.peer_id == peer_id))
    }

    fn to_live(&self, n: CoordinatorNode, now_ms: u64) -> LiveNode {
        // saturating_sub guards against clock skew making last_seen > now.
        let seconds_since_seen = now_ms.saturating_sub(n.last_seen_ms) / 1000;
        LiveNode {
            online: seconds_since_seen < self.online_ttl_secs,
            peer_id: n.peer_id,
            last_seen_ms: n.last_seen_ms,
            seconds_since_seen,
            models: n.models,
            max_concurrent_jobs: n.max_concurrent_jobs,
            multiaddrs: n.multiaddrs,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
