use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn connect(url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(url)
        .await?;
    Ok(pool)
}

// ── Row types matching the coordinator's schema ──────────────────────────────

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ReceiptRow {
    pub request_id: uuid::Uuid,
    pub receipt_json: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub walrus_blob_id: Option<String>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct PaymentRow {
    pub id: uuid::Uuid,
    pub request_id: uuid::Uuid,
    pub payee_peer_id: String,
    pub payee_sui_address: String,
    pub amount_nanox: i64,
    pub status: String,
    pub tx_digest: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Full record returned by GET /api/r/:request_id
#[derive(Debug, serde::Serialize)]
pub struct RequestRecord {
    pub receipt: ReceiptRow,
    pub payments: Vec<PaymentRow>,
}

pub async fn get_receipt(
    pool: &PgPool,
    request_id: uuid::Uuid,
) -> Result<Option<ReceiptRow>> {
    let row = sqlx::query_as::<_, ReceiptRow>(
        "SELECT request_id, receipt_json, created_at, walrus_blob_id
         FROM routing_receipts WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_payments(
    pool: &PgPool,
    request_id: uuid::Uuid,
) -> Result<Vec<PaymentRow>> {
    let rows = sqlx::query_as::<_, PaymentRow>(
        "SELECT id, request_id, payee_peer_id, payee_sui_address, amount_nanox,
                status, tx_digest, created_at, submitted_at, confirmed_at
         FROM payments WHERE request_id = $1 ORDER BY created_at",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn recent_receipts(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<ReceiptRow>> {
    let rows = sqlx::query_as::<_, ReceiptRow>(
        "SELECT request_id, receipt_json, created_at, walrus_blob_id
         FROM routing_receipts ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Receipts older than `minutes` that haven't been archived yet.
pub async fn unarchived_older_than(
    pool: &PgPool,
    minutes: i64,
) -> Result<Vec<ReceiptRow>> {
    let rows = sqlx::query_as::<_, ReceiptRow>(
        "SELECT request_id, receipt_json, created_at, walrus_blob_id
         FROM routing_receipts
         WHERE walrus_blob_id IS NULL
           AND created_at < NOW() - ($1 || ' minutes')::INTERVAL
         ORDER BY created_at
         LIMIT 500",
    )
    .bind(minutes)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn set_walrus_blob_id(
    pool: &PgPool,
    request_id: uuid::Uuid,
    blob_id: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE routing_receipts SET walrus_blob_id = $1 WHERE request_id = $2",
    )
    .bind(blob_id)
    .bind(request_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Receipts served by a given peer_id (from receipt_json payloads).
pub async fn receipts_for_node(
    pool: &PgPool,
    peer_id: &str,
    limit: i64,
) -> Result<Vec<ReceiptRow>> {
    let rows = sqlx::query_as::<_, ReceiptRow>(
        "SELECT request_id, receipt_json, created_at, walrus_blob_id
         FROM routing_receipts
         WHERE receipt_json->>'primary_peer_id' = $1
            OR receipt_json->'helper_peer_ids' ? $1
         ORDER BY created_at DESC
         LIMIT $2",
    )
    .bind(peer_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
