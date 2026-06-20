//! HTTP API routes.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::coordinator::{CoordinatorClient, LiveNode};
use crate::{archive::WalrusClient, db};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub walrus: Arc<WalrusClient>,
    pub coordinator: Arc<CoordinatorClient>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/r/{request_id}", get(get_request))
        .route("/api/nodes", get(list_nodes))
        .route("/api/nodes/{peer_id}", get(get_node))
        .route("/api/recent", get(recent))
}

// ── /health ──────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ── GET /api/r/:request_id ────────────────────────────────────────────────────

async fn get_request(
    State(state): State<AppState>,
    Path(request_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let receipt = db::get_receipt(&state.pool, request_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let payments = db::get_payments(&state.pool, request_id).await?;
    Ok(Json(db::RequestRecord { receipt, payments }))
}

// ── GET /api/nodes — live node status list ────────────────────────────────────

#[derive(Serialize)]
struct NodeStatus {
    #[serde(flatten)]
    live: LiveNode,
    /// Jobs served as primary (lifetime), from routing receipts.
    jobs_served: i64,
}

/// Live node status: who is connected to the coordinator right now, with
/// an `online` flag derived from last_seen freshness, plus each node's
/// lifetime job count. Source of truth for "who's online" is the
/// coordinator's live peer registry.
async fn list_nodes(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let live = state.coordinator.live_nodes().await?;

    // One grouped query for all job counts, then attach.
    let counts: std::collections::HashMap<String, i64> = db::node_job_counts(&state.pool)
        .await?
        .into_iter()
        .collect();

    let nodes: Vec<NodeStatus> = live
        .into_iter()
        .map(|n| {
            let jobs_served = counts.get(&n.peer_id).copied().unwrap_or(0);
            NodeStatus {
                live: n,
                jobs_served,
            }
        })
        .collect();

    Ok(Json(nodes))
}

// ── GET /api/nodes/:peer_id — live details + history ──────────────────────────

#[derive(Serialize)]
struct NodeProfile {
    peer_id: String,
    /// Live status from the coordinator's peer registry; null if the
    /// node is not currently known to the coordinator (offline/evicted).
    live: Option<LiveNode>,
    jobs_served: i64,
    recent_receipts: Vec<db::ReceiptRow>,
}

async fn get_node(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // Live status is best-effort: if the coordinator is unreachable we
    // still return the historical view rather than failing the request.
    let live = match state.coordinator.live_node(&peer_id).await {
        Ok(live) => live,
        Err(e) => {
            tracing::warn!(err = %e, %peer_id, "coordinator live lookup failed");
            None
        }
    };
    let jobs_served = db::node_job_count(&state.pool, &peer_id).await?;
    let recent_receipts = db::receipts_for_node(&state.pool, &peer_id, 50).await?;
    Ok(Json(NodeProfile {
        peer_id,
        live,
        jobs_served,
        recent_receipts,
    }))
}

// ── GET /api/recent ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Pagination {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    20
}

async fn recent(
    State(state): State<AppState>,
    Query(p): Query<Pagination>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = p.limit.clamp(1, 100);
    let rows = db::recent_receipts(&state.pool, limit, p.offset).await?;
    Ok(Json(rows))
}

// ── Error type ────────────────────────────────────────────────────────────────

enum ApiError {
    NotFound,
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "not found" }))).into_response(),
            Self::Internal(e) => {
                tracing::error!(err = %e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "internal error" }))).into_response()
            }
        }
    }
}
