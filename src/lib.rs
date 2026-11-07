pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod storage;

use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: PgPool,
    pub storage: Arc<Storage>,
}

pub fn app(state: AppState) -> Router {
    let upload_root = state.storage.local_root().map(|path| path.to_path_buf());
    let router = Router::new()
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
        .merge(crate::routes::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    if let Some(root) = upload_root {
        router.nest_service("/uploads", ServeDir::new(root))
    } else {
        router
    }
}

pub fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.to_lowercase().chars() {
        let mapped = match ch {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'й' => "i",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "c",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "sch",
            'ъ' | 'ь' => "",
            'ы' => "y",
            'э' => "e",
            'ю' => "yu",
            'я' => "ya",
            _ => "",
        };
        if mapped.is_empty() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
            } else if !out.ends_with('-') {
                out.push('-');
            }
        } else if mapped.chars().all(|part| part.is_ascii_alphanumeric()) {
            out.push_str(mapped);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}
