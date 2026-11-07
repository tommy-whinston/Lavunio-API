use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use lavunio_api::auth;
use lavunio_api::config::Config;
use lavunio_api::db;
use lavunio_api::routes;
use lavunio_api::storage::Storage;
use lavunio_api::AppState;
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,lavunio_api=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let pool = db::pool(&config.database_url);
    let storage = Arc::new(Storage::new(
        config.storage_dir.clone(),
        config.public_base_url.clone(),
    ));
    let state = AppState {
        config: Arc::new(config.clone()),
        pool: pool.clone(),
        storage: storage.clone(),
    };

    db::migrate(&pool).await;
    auth::ensure_bootstrap(&pool, config.bootstrap_token.as_deref()).await;

    let app = Router::new()
        .route(
            "/",
            get(|| async {
                Json(json!({
                    "name": "Lavunio API",
                    "version": env!("CARGO_PKG_VERSION"),
                    "docs": "/v1/health"
                }))
            }),
        )
        .merge(routes::router())
        .nest_service("/uploads", ServeDir::new(storage.root()))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind {address}: {error}"));
    tracing::info!("Lavunio API listening on http://{address}");
    axum::serve(listener, app).await.expect("API server failed");
}
