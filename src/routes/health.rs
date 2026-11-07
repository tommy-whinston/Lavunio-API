use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

async fn database_ok(state: &AppState) -> bool {
    tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool),
    )
    .await
    .map(|result| result.is_ok())
    .unwrap_or(false)
}

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let started = Instant::now();
    let database_ok = database_ok(&state).await;
    let database_latency_ms = started.elapsed().as_millis() as u64;

    Json(json!({
        "status": if database_ok { "ok" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": if database_ok { "ok" } else { "down" },
        "databaseLatencyMs": database_latency_ms,
        "uptimeSeconds": state.metrics.uptime_secs(),
        "storage": state.storage.kind().as_str(),
        "pool": {
            "size": state.pool.size() as u64,
            "idle": state.pool.num_idle() as u64,
        }
    }))
}

pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let database_ok = database_ok(&state).await;
    let pending =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM releases WHERE status = 'pending'")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);

    let body = format!(
        "\
# HELP lavunio_uptime_seconds API uptime in seconds
# TYPE lavunio_uptime_seconds gauge
lavunio_uptime_seconds {uptime}
# HELP lavunio_requests_total Total HTTP requests
# TYPE lavunio_requests_total counter
lavunio_requests_total {requests}
# HELP lavunio_errors_total Total HTTP responses with status >= 400
# TYPE lavunio_errors_total counter
lavunio_errors_total {errors}
# HELP lavunio_db_up Database availability (1 = up, 0 = down)
# TYPE lavunio_db_up gauge
lavunio_db_up {db_up}
# HELP lavunio_db_pool_size Database pool size
# TYPE lavunio_db_pool_size gauge
lavunio_db_pool_size {pool_size}
# HELP lavunio_db_pool_idle Idle database connections
# TYPE lavunio_db_pool_idle gauge
lavunio_db_pool_idle {pool_idle}
# HELP lavunio_pending_releases Releases waiting for moderation
# TYPE lavunio_pending_releases gauge
lavunio_pending_releases {pending}
",
        uptime = state.metrics.uptime_secs(),
        requests = state.metrics.requests(),
        errors = state.metrics.errors(),
        db_up = if database_ok { 1 } else { 0 },
        pool_size = state.pool.size() as u64,
        pool_idle = state.pool.num_idle() as u64,
        pending = pending,
    );

    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
