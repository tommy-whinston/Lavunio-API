use std::sync::Arc;

use lavunio_api::auth;
use lavunio_api::config::Config;
use lavunio_api::db;
use lavunio_api::storage::Storage;
use lavunio_api::AppState;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,lavunio_api=debug".into());
    let json_logs = std::env::var("LOG_FORMAT")
        .map(|value| value.trim().eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if json_logs {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().compact())
            .init();
    }

    let config = Config::from_env();
    let pool = db::pool(&config.database_url);
    let storage = Arc::new(
        Storage::new(&config).unwrap_or_else(|error| panic!("storage setup failed: {error}")),
    );
    let state = AppState::new(Arc::new(config.clone()), pool.clone(), storage);

    db::migrate(&pool).await;
    auth::ensure_bootstrap(&pool, config.bootstrap_token.as_deref()).await;

    let app = lavunio_api::app(state);

    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind {address}: {error}"));
    tracing::info!(
        "Lavunio API listening on http://{address} (storage: {:?})",
        config.storage_kind
    );
    axum::serve(listener, app).await.expect("API server failed");
}
