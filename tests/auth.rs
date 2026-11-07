use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use lavunio_api::config::Config;
use lavunio_api::storage::Storage;
use lavunio_api::{app, AppState};

fn test_state(pool: PgPool) -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp storage");
    let config = Config::local(String::new(), dir.path().to_path_buf());
    let storage = Storage::new(&config).expect("local storage");
    (
        AppState {
            config: Arc::new(config),
            pool,
            storage: Arc::new(storage),
        },
        dir,
    )
}

async fn send(
    app: Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(value) => Body::from(serde_json::to_vec(&value).expect("json body")),
        None => Body::empty(),
    };
    let request = builder
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .expect("request");
    let response = app.oneshot(request).await.expect("request result");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn register(app: &Router, email: &str, username: &str, password: &str) -> Value {
    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/register",
        None,
        Some(json!({
            "email": email,
            "username": username,
            "password": password
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[sqlx::test]
async fn register_login_refresh_logout_flow(pool: PgPool) {
    let (state, _dir) = test_state(pool);
    let app = app(state);
    let password = "correct horse battery";

    let registered = register(&app, "Steve@Example.com", "steve", password).await;
    assert_eq!(registered["user"]["email"], "steve@example.com");
    assert_eq!(registered["user"]["emailVerified"], false);
    assert_eq!(registered["tokens"]["tokenType"], "Bearer");
    let access = registered["tokens"]["accessToken"]
        .as_str()
        .expect("access token")
        .to_string();
    let verification_token = registered["verificationToken"]
        .as_str()
        .expect("dev verification token")
        .to_string();

    let (status, me) = send(app.clone(), Method::GET, "/v1/me", Some(&access), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["username"], "steve");

    let (status, _) = send(app.clone(), Method::GET, "/v1/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, wrong) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/login",
        None,
        Some(json!({ "email": "steve@example.com", "password": "wrong password" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{wrong}");

    let (status, login) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/login",
        None,
        Some(json!({ "email": "STEVE@example.com", "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let login_access = login["tokens"]["accessToken"]
        .as_str()
        .expect("login access")
        .to_string();
    let login_refresh = login["tokens"]["refreshToken"]
        .as_str()
        .expect("login refresh")
        .to_string();

    let (status, verified) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/verify-email",
        None,
        Some(json!({ "token": verification_token })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{verified}");

    let (status, me) = send(
        app.clone(),
        Method::GET,
        "/v1/me",
        Some(&login_access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["emailVerified"], true);

    let (status, refreshed) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/refresh",
        None,
        Some(json!({ "refreshToken": login_refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refreshed}");
    let new_access = refreshed["tokens"]["accessToken"]
        .as_str()
        .expect("new access")
        .to_string();
    let new_refresh = refreshed["tokens"]["refreshToken"]
        .as_str()
        .expect("new refresh")
        .to_string();
    assert_ne!(new_access, login_access);
    assert_ne!(new_refresh, login_refresh);

    let (status, _) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/refresh",
        None,
        Some(json!({ "refreshToken": login_refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, logged_out) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/logout",
        Some(&new_access),
        Some(json!({ "refreshToken": new_refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{logged_out}");

    let (status, _) = send(app, Method::GET, "/v1/me", Some(&new_access), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn password_reset_revokes_sessions(pool: PgPool) {
    let (state, _dir) = test_state(pool);
    let app = app(state);

    let registered = register(&app, "reset@example.com", "reset_user", "old password 123").await;
    let old_access = registered["tokens"]["accessToken"]
        .as_str()
        .expect("access token")
        .to_string();

    let (status, forgot) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/password/forgot",
        None,
        Some(json!({ "email": "reset@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{forgot}");
    let reset_token = forgot["resetToken"]
        .as_str()
        .expect("dev reset token")
        .to_string();

    let (status, reset) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/password/reset",
        None,
        Some(json!({ "token": reset_token, "password": "new password 456" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reset}");

    let (status, _) = send(app.clone(), Method::GET, "/v1/me", Some(&old_access), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/login",
        None,
        Some(json!({ "email": "reset@example.com", "password": "old password 123" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, login) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/login",
        None,
        Some(json!({ "email": "reset@example.com", "password": "new password 456" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");

    let (status, _) = send(
        app,
        Method::POST,
        "/v1/auth/password/reset",
        None,
        Some(json!({ "token": reset_token, "password": "another password 789" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn duplicate_email_and_username_rejected(pool: PgPool) {
    let (state, _dir) = test_state(pool);
    let app = app(state);

    register(&app, "taken@example.com", "taken_user", "password 123").await;

    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/register",
        None,
        Some(json!({
            "email": "TAKEN@example.com",
            "username": "another_user",
            "password": "password 123"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/register",
        None,
        Some(json!({
            "email": "another@example.com",
            "username": "TAKEN_USER",
            "password": "password 123"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[sqlx::test]
async fn registration_validates_input(pool: PgPool) {
    let (state, _dir) = test_state(pool);
    let app = app(state);

    for payload in [
        json!({ "email": "not-an-email", "username": "player", "password": "password 123" }),
        json!({ "email": "ok@example.com", "username": "player", "password": "short" }),
        json!({ "email": "ok@example.com", "username": "-bad-", "password": "password 123" }),
    ] {
        let (status, body) = send(
            app.clone(),
            Method::POST,
            "/v1/auth/register",
            None,
            Some(payload),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
}
