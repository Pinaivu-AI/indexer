use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod api;
mod archive;
mod db;
mod config;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pinaivu_indexer=info,tower_http=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();
    let cfg = config::Config::from_env()?;

    let pool = db::connect(&cfg.database_url)
        .await
        .context("connect postgres")?;
    tracing::info!("postgres connected");

    let walrus = Arc::new(archive::WalrusClient::new(cfg.walrus_publisher_url.clone()));

    // Archive cron: every 5 minutes pack receipts older than ARCHIVE_AFTER_MINUTES to Walrus.
    let sched = JobScheduler::new().await?;
    {
        let pool2 = pool.clone();
        let walrus2 = walrus.clone();
        sched
            .add(Job::new_async("0 */5 * * * *", move |_, _| {
                let p = pool2.clone();
                let w = walrus2.clone();
                Box::pin(async move {
                    if let Err(e) = archive::run_archive_job(&p, &w).await {
                        tracing::error!(err = %e, "archive job failed");
                    }
                })
            })?)
            .await?;
    }
    sched.start().await?;
    tracing::info!("archive cron scheduler started");

    let state = api::AppState { pool, walrus };

    let router = Router::new()
        .merge(api::routes())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let bind = cfg.bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    tracing::info!(addr = %bind, "indexer listening");

    axum::serve(listener, router).await?;
    Ok(())
}
