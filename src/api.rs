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

use crate::{archive::WalrusClient, db};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub walrus: Arc<WalrusClient>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/r/{request_id}", get(get_request))
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

// ── GET /api/nodes/:peer_id ───────────────────────────────────────────────────

#[derive(Serialize)]
struct NodeProfile {
    peer_id: String,
    jobs_served: usize,
    recent_receipts: Vec<db::ReceiptRow>,
}

async fn get_node(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let receipts = db::receipts_for_node(&state.pool, &peer_id, 50).await?;
    let jobs_served = receipts.len();
    Ok(Json(NodeProfile {
        peer_id,
        jobs_served,
        recent_receipts: receipts,
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
