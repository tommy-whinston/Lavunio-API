pub mod admin;
pub mod auth;
pub mod catalog;
pub mod health;
pub mod publisher;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/health", get(health::health))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/verify-email", post(auth::verify_email))
        .route(
            "/v1/auth/verify-email/resend",
            post(auth::resend_verification),
        )
        .route("/v1/auth/password/forgot", post(auth::forgot_password))
        .route("/v1/auth/password/reset", post(auth::reset_password))
        .route("/v1/me", get(auth::me))
        .route("/v1/me/identities", get(auth::identities))
        .route("/v1/catalog", get(catalog::catalog))
        .route("/v1/modes/{slug}", get(catalog::mode_detail))
        .route("/v1/modes/{slug}/releases", get(catalog::releases))
        .route("/v1/authors/{slug}", get(catalog::author))
        .route("/v1/publisher/modes", post(publisher::create_mode))
        .route(
            "/v1/publisher/modes/{id}/releases",
            post(publisher::create_release),
        )
        .route(
            "/v1/publisher/releases/{id}/assets/upload-url",
            post(publisher::upload_url),
        )
        .route(
            "/v1/publisher/releases/{release_id}/assets/{asset_id}/upload",
            put(publisher::upload_asset).layer(DefaultBodyLimit::max(512 * 1024 * 1024)),
        )
        .route(
            "/v1/publisher/releases/{release_id}/assets/{asset_id}/complete",
            post(publisher::complete_asset),
        )
        .route(
            "/v1/publisher/releases/{id}/submit",
            post(publisher::submit_release),
        )
        .route("/v1/admin/moderation/queue", get(admin::queue))
        .route("/v1/admin/moderation/{id}/approve", post(admin::approve))
        .route("/v1/admin/moderation/{id}/reject", post(admin::reject))
}

pub fn validate_slug(value: &str) -> bool {
    let length = value.len();
    (3..=64).contains(&length)
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}
