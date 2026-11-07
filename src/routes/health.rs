use std::time::Duration;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let database = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.pool),
    )
    .await;

    let database = match database {
        Ok(Ok(_)) => "ok",
        _ => "down",
    };

    Json(json!({
        "status": if database == "ok" { "ok" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": database
    }))
}
