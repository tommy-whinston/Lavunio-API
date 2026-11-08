use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use image::{ImageEncoder, Rgba, RgbaImage};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use lavunio_api::auth;
use lavunio_api::config::Config;
use lavunio_api::storage::Storage;
use lavunio_api::{app, AppState};

fn test_state(pool: PgPool) -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp storage");
    let config = Config::local(String::new(), dir.path().to_path_buf());
    let storage = Storage::new(&config).expect("local storage");
    (
        AppState::new(Arc::new(config), pool, Arc::new(storage)),
        dir,
    )
}

async fn prepare(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("migrations");
}

async fn create_user_token(pool: &PgPool, username: &str, role: &str) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, display_name, role) VALUES ($1, $2, $2, $3)")
        .bind(user_id)
        .bind(username)
        .bind(role)
        .execute(pool)
        .await
        .expect("insert user");

    let token = format!("test_{}", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, scopes) VALUES ($1, $2, 'test', $3, '[\"publisher\",\"admin\"]'::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(auth::hash_token(&token))
    .execute(pool)
    .await
    .expect("insert token");

    (user_id, token)
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba([229, 50, 43, 255]));
    let mut output = Vec::new();
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .expect("png encode");
    output
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

async fn send_bytes(
    app: Router,
    method: Method,
    uri: &str,
    token: &str,
    bytes: Bytes,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(bytes))
        .expect("request");
    let response = app.oneshot(request).await.expect("request result");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn send_text(app: Router, uri: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("request");
    let response = app.oneshot(request).await.expect("request result");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

#[sqlx::test]
async fn health_and_metrics_report_state(pool: PgPool) {
    prepare(&pool).await;
    let (state, _dir) = test_state(pool);
    let app = app(state);

    let (status, health) = send(app.clone(), Method::GET, "/v1/health", None, None).await;
    assert_eq!(status, StatusCode::OK, "{health}");
    assert_eq!(health["database"], "ok");
    assert_eq!(health["storage"], "local");
    assert!(health["uptimeSeconds"].as_u64().is_some());
    assert!(health["databaseLatencyMs"].as_u64().is_some());
    assert!(health["pool"]["size"].as_u64().unwrap_or(0) >= 1);

    let (status, metrics) = send_text(app, "/v1/metrics").await;
    assert_eq!(status, StatusCode::OK, "{metrics}");
    assert!(metrics.contains("lavunio_uptime_seconds"));
    assert!(metrics.contains("lavunio_requests_total"));
    assert!(metrics.contains("lavunio_db_up 1"));
    assert!(metrics.contains("lavunio_pending_releases"));
}

#[sqlx::test]
async fn catalog_returns_only_published_modes(pool: PgPool) {
    prepare(&pool).await;
    let (user_id, _) = create_user_token(&pool, "publisher", "admin").await;

    sqlx::query(
        "INSERT INTO modes (id, slug, title, status, author_id) VALUES ($1, 'draft-mode', 'Draft', 'draft', $2)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("draft mode");

    let published_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO modes (id, slug, title, status, author_id) VALUES ($1, 'published-mode', 'Published', 'published', $2)",
    )
    .bind(published_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("published mode");

    sqlx::query(
        "INSERT INTO releases (id, mode_id, version, status, published_at) VALUES ($1, $2, '1.0.0', 'published', now())",
    )
    .bind(Uuid::new_v4())
    .bind(published_id)
    .execute(&pool)
    .await
    .expect("release");

    let (state, _dir) = test_state(pool);
    let app = app(state);
    let (status, body) = send(app.clone(), Method::GET, "/v1/catalog", None, None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["slug"], "published-mode");

    let (status, sections) = send(app, Method::GET, "/v1/collections", None, None).await;
    assert_eq!(status, StatusCode::OK, "{sections}");
    let sections = sections.as_array().expect("sections array");
    for id in ["featured", "popular", "new", "updated"] {
        let section = sections
            .iter()
            .find(|section| section["id"] == id)
            .unwrap_or_else(|| panic!("missing section {id}"));
        assert_eq!(section["modes"][0]["slug"], "published-mode");
    }
}

#[sqlx::test]
async fn follows_and_feed_return_author_updates(pool: PgPool) {
    prepare(&pool).await;
    let (author_id, author_token) = create_user_token(&pool, "follow-author", "author").await;
    let (_, token) = create_user_token(&pool, "follow-player", "player").await;

    let mode_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO modes (id, slug, title, status, author_id) VALUES ($1, 'follow-mode', 'Follow', 'published', $2)",
    )
    .bind(mode_id)
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("mode");
    sqlx::query(
        "INSERT INTO releases (id, mode_id, version, status, published_at) VALUES ($1, $2, '1.2.3', 'published', now())",
    )
    .bind(Uuid::new_v4())
    .bind(mode_id)
    .execute(&pool)
    .await
    .expect("release");

    let (state, _dir) = test_state(pool);
    let app = app(state);

    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/follows/follow-author",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, follows) = send(
        app.clone(),
        Method::GET,
        "/v1/me/follows",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{follows}");
    assert_eq!(follows.as_array().unwrap().len(), 1);
    assert_eq!(follows[0]["username"], "follow-author");
    assert_eq!(follows[0]["followersCount"], 1);

    let (status, feed) = send(
        app.clone(),
        Method::GET,
        "/v1/me/feed?limit=10",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{feed}");
    assert_eq!(feed.as_array().unwrap().len(), 1);
    assert_eq!(feed[0]["modeSlug"], "follow-mode");
    assert_eq!(feed[0]["version"], "1.2.3");

    let (status, self_follow) = send(
        app.clone(),
        Method::POST,
        "/v1/follows/follow-author",
        Some(&author_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{self_follow}");

    let (status, unfollowed) = send(
        app.clone(),
        Method::DELETE,
        "/v1/follows/follow-author",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{unfollowed}");

    let (status, follows) = send(app, Method::GET, "/v1/me/follows", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{follows}");
    assert_eq!(follows, json!([]));
}

#[sqlx::test]
async fn news_store_and_mode_announcements(pool: PgPool) {
    prepare(&pool).await;
    let (author_id, author_token) = create_user_token(&pool, "news-author", "author").await;
    let (_, admin_token) = create_user_token(&pool, "news-admin", "admin").await;
    let (_, player_token) = create_user_token(&pool, "news-player", "player").await;

    let mode_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO modes (id, slug, title, status, author_id) VALUES ($1, 'news-mode', 'News Mode', 'published', $2)",
    )
    .bind(mode_id)
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("mode");

    let (state, _dir) = test_state(pool);
    let app = app(state);

    let (status, published) = send(
        app.clone(),
        Method::POST,
        "/v1/news",
        Some(&admin_token),
        Some(json!({ "title": "Запуск магазина", "body": "Магазин Lavunio открыт." })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");

    let (status, store_news) = send(app.clone(), Method::GET, "/v1/news", None, None).await;
    assert_eq!(status, StatusCode::OK, "{store_news}");
    assert_eq!(store_news.as_array().unwrap().len(), 1);
    assert_eq!(store_news[0]["title"], "Запуск магазина");
    assert!(store_news[0]["modeId"].is_null());

    let (status, mode_news) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/publisher/modes/{mode_id}/news"),
        Some(&author_token),
        Some(json!({ "title": "Новая версия", "body": "Добавлены новые механики." })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mode_news}");

    let (status, mode_news) = send(
        app.clone(),
        Method::GET,
        "/v1/modes/news-mode/news",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mode_news}");
    assert_eq!(mode_news.as_array().unwrap().len(), 1);
    assert_eq!(mode_news[0]["title"], "Новая версия");
    assert_eq!(mode_news[0]["modeSlug"], "news-mode");

    let (status, denied) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/publisher/modes/{mode_id}/news"),
        Some(&player_token),
        Some(json!({ "title": "Чужая новость", "body": "Не должно пройти." })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let news_id = store_news[0]["id"].as_str().expect("news id");
    let (status, deleted) = send(
        app.clone(),
        Method::DELETE,
        &format!("/v1/news/{news_id}"),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");

    let (status, store_news) = send(app, Method::GET, "/v1/news", None, None).await;
    assert_eq!(status, StatusCode::OK, "{store_news}");
    assert_eq!(store_news, json!([]));
}

#[sqlx::test]
async fn publisher_flow_uploads_submits_and_publishes(pool: PgPool) {
    prepare(&pool).await;
    let (_, token) = create_user_token(&pool, "admin", "admin").await;
    let (state, _dir) = test_state(pool);
    let app = app(state);

    let (status, mode) = send(
        app.clone(),
        Method::POST,
        "/v1/publisher/modes",
        Some(&token),
        Some(json!({
            "slug": "api-test",
            "title": "API Test",
            "shortDescription": "Test mode",
            "description": "Created by integration test",
            "mcVersion": "1.12.2",
            "loader": "forge",
            "trailerUrl": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "tags": ["test", "api"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mode}");
    let mode_id = mode["id"].as_str().expect("mode id");

    let (status, release) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/publisher/modes/{mode_id}/releases"),
        Some(&token),
        Some(json!({ "version": "0.1.0", "changelog": "First release" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{release}");
    let release_id = release["id"].as_str().expect("release id");

    let (status, upload) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/publisher/releases/{release_id}/assets/upload-url"),
        Some(&token),
        Some(json!({
            "kind": "archive",
            "fileName": "build.zip",
            "size": 11,
            "manifest": {
                "version": "0.1.0",
                "files": {
                    "mods/test.jar": { "sha256": "abc", "size": 11 }
                },
                "archive": {
                    "path": "build.zip",
                    "size": 11,
                    "sha256": null,
                    "files": 1,
                    "bytes": 11,
                    "parts": []
                }
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{upload}");
    assert_eq!(upload["storage"], "local");
    let upload_url = upload["uploadUrl"].as_str().expect("upload url");
    let upload_path = upload_url
        .strip_prefix("http://test")
        .expect("test public base url");

    let (status, asset) = send_bytes(
        app.clone(),
        Method::PUT,
        upload_path,
        &token,
        Bytes::from_static(b"hello world"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{asset}");
    assert_eq!(asset["size"], 11);
    assert_eq!(asset["manifest"]["files"]["mods/test.jar"]["size"], 11);
    assert!(asset["url"].as_str().unwrap().contains("/uploads/"));

    let (status, shot_upload) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/publisher/releases/{release_id}/assets/upload-url"),
        Some(&token),
        Some(json!({
            "kind": "screenshot",
            "fileName": "screen.png",
            "size": 9
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{shot_upload}");
    let shot_url = shot_upload["uploadUrl"].as_str().expect("shot upload url");
    let shot_path = shot_url
        .strip_prefix("http://test")
        .expect("test public base url");
    let (status, shot) = send_bytes(
        app.clone(),
        Method::PUT,
        shot_path,
        &token,
        Bytes::from_static(b"shot-data"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{shot}");
    assert!(shot["url"].as_str().unwrap().contains("/uploads/"));

    let (status, submitted) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/publisher/releases/{release_id}/submit"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    assert_eq!(submitted["status"], "pending");

    let (status, queue) = send(
        app.clone(),
        Method::GET,
        "/v1/admin/moderation/queue",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_eq!(queue.as_array().unwrap().len(), 1);

    let (status, approved) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/moderation/{release_id}/approve"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["status"], "published");

    let (status, catalog) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?q=API%20Test",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["total"], 1);
    assert_eq!(catalog["items"][0]["slug"], "api-test");
    assert_eq!(catalog["items"][0]["mcVersion"], "1.12.2");
    assert_eq!(
        catalog["items"][0]["trailerUrl"],
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert_eq!(
        catalog["items"][0]["screenshots"].as_array().unwrap().len(),
        1
    );

    let (status, mine) = send(
        app.clone(),
        Method::GET,
        "/v1/publisher/modes",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine.as_array().unwrap().len(), 1);
    assert_eq!(mine[0]["slug"], "api-test");
    assert_eq!(mine[0]["releases"][0]["version"], "0.1.0");
    assert_eq!(mine[0]["releases"][0]["status"], "published");
    assert_eq!(
        mine[0]["releases"][0]["assets"][0]["manifest"]["files"]["mods/test.jar"]["size"],
        11
    );

    for event in ["download", "install", "update"] {
        let (status, body) = send(
            app.clone(),
            Method::POST,
            "/v1/modes/api-test/events",
            Some(&token),
            Some(json!({ "event": event, "version": "0.1.0" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, stats) = send(
        app.clone(),
        Method::GET,
        &format!("/v1/publisher/modes/{mode_id}/stats"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["downloads"], 1);
    assert_eq!(stats["installs"], 1);
    assert_eq!(stats["updates"], 1);
    assert_eq!(stats["uniqueUsers"], 1);

    let (status, filtered) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?mcVersion=1.12.2&loader=forge&sort=installs",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["items"][0]["installs"], 1);

    let (status, cover) = send_bytes(
        app.clone(),
        Method::PUT,
        &format!("/v1/publisher/modes/{mode_id}/cover"),
        &token,
        Bytes::from(png_bytes(512, 256)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cover}");
    let cover_url = cover["coverUrl"].as_str().expect("cover url").to_string();
    assert!(cover_url.contains("/uploads/covers/"));

    let (status, mine) = send(
        app.clone(),
        Method::GET,
        "/v1/publisher/modes",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine[0]["coverUrl"], cover_url.as_str());
    assert_eq!(mine[0]["stats"]["downloads"], 1);
    assert_eq!(mine[0]["stats"]["uniqueUsers"], 1);

    let (status, catalog) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?q=API%20Test",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["items"][0]["coverUrl"], cover_url.as_str());

    let (status, deleted) = send(
        app,
        Method::DELETE,
        &format!("/v1/publisher/modes/{mode_id}/cover"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
}

#[sqlx::test]
async fn moderation_rejects_hides_and_logs_actions(pool: PgPool) {
    prepare(&pool).await;
    let (author_id, author_token) = create_user_token(&pool, "mod-author", "author").await;
    let (_, admin_token) = create_user_token(&pool, "mod-admin", "admin").await;

    let mode_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO modes (id, slug, title, status, author_id) VALUES ($1, 'moderated-mode', 'Moderated Mode', 'pending', $2)",
    )
    .bind(mode_id)
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("mode");

    let release_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO releases (id, mode_id, version, status, changelog) VALUES ($1, $2, '1.0.0', 'pending', 'Need review')",
    )
    .bind(release_id)
    .bind(mode_id)
    .execute(&pool)
    .await
    .expect("release");

    let (state, _dir) = test_state(pool.clone());
    let app = app(state);

    let (status, queue) = send(
        app.clone(),
        Method::GET,
        "/v1/admin/moderation/queue",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_eq!(queue[0]["authorUsername"], "mod-author");
    assert_eq!(queue[0]["changelog"], "Need review");

    let (status, body) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/moderation/{release_id}/reject"),
        Some(&admin_token),
        Some(json!({ "note": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let reason = "Сборка падает при запуске";
    let (status, body) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/moderation/{release_id}/reject"),
        Some(&admin_token),
        Some(json!({ "note": reason })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, mine) = send(
        app.clone(),
        Method::GET,
        "/v1/publisher/modes",
        Some(&author_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine[0]["status"], "draft");
    assert_eq!(mine[0]["releases"][0]["rejectionNote"], reason);

    let second_release = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO releases (id, mode_id, version, status, changelog) VALUES ($1, $2, '1.0.1', 'pending', 'Fixed')",
    )
    .bind(second_release)
    .bind(mode_id)
    .execute(&pool)
    .await
    .expect("second release");

    let (status, body) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/moderation/{second_release}/approve"),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, catalog) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?q=Moderated",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["total"], 1);

    let (status, body) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/modes/{mode_id}/hide"),
        Some(&admin_token),
        Some(json!({ "note": "Жалоба на контент" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, catalog) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?q=Moderated",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["total"], 0);

    let (status, modes) = send(
        app.clone(),
        Method::GET,
        "/v1/admin/modes",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{modes}");
    assert_eq!(modes[0]["status"], "hidden");

    let (status, body) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/modes/{mode_id}/unhide"),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, catalog) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?q=Moderated",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["total"], 1);

    let (status, history) = send(
        app,
        Method::GET,
        "/v1/admin/moderation/history?limit=20",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    let actions: Vec<&str> = history
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["action"].as_str())
        .collect();
    for expected in ["reject", "approve", "hide", "unhide"] {
        assert!(actions.contains(&expected), "missing {expected}: {history}");
    }
    assert!(history
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["note"] == reason));
}

#[sqlx::test]
async fn wishlist_and_reviews_work(pool: PgPool) {
    prepare(&pool).await;
    let (user_id, token) = create_user_token(&pool, "community-user", "player").await;
    let (_, author_token) = create_user_token(&pool, "community-author", "author").await;
    let (author_id, _) = create_user_token(&pool, "community-author-2", "author").await;
    let (_, admin_token) = create_user_token(&pool, "community-admin", "admin").await;

    sqlx::query(
        "INSERT INTO modes (id, slug, title, description, status, author_id) VALUES ($1, 'community-mode', 'Community Mode', 'Test', 'published', $2)",
    )
    .bind(Uuid::new_v4())
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("mode");

    let (state, _dir) = test_state(pool);
    let app = app(state);

    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/me/wishlist/community-mode",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, list) = send(
        app.clone(),
        Method::GET,
        "/v1/me/wishlist",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list, json!(["community-mode"]));

    let (status, body) = send(
        app.clone(),
        Method::DELETE,
        "/v1/me/wishlist/community-mode",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, list) = send(
        app.clone(),
        Method::GET,
        "/v1/me/wishlist",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list, json!([]));

    let (status, empty) = send(
        app.clone(),
        Method::GET,
        "/v1/modes/community-mode/reviews",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["count"], 0);

    let (status, created) = send(
        app.clone(),
        Method::POST,
        "/v1/modes/community-mode/reviews",
        Some(&token),
        Some(json!({ "rating": 5, "text": "Отличный режим" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["count"], 1);
    assert_eq!(created["averageRating"], 5.0);
    assert_eq!(created["items"][0]["authorUsername"], "community-user");

    let (status, updated) = send(
        app.clone(),
        Method::POST,
        "/v1/modes/community-mode/reviews",
        Some(&token),
        Some(json!({ "rating": 3, "text": "Передумал" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["count"], 1);
    assert_eq!(updated["averageRating"], 3.0);

    let (status, catalog) = send(
        app.clone(),
        Method::GET,
        "/v1/catalog?q=Community",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["items"][0]["rating"], 3.0);
    assert_eq!(catalog["items"][0]["reviewsCount"], 1);

    let (status, invalid) = send(
        app.clone(),
        Method::POST,
        "/v1/modes/community-mode/reviews",
        Some(&token),
        Some(json!({ "rating": 6, "text": "bad" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");

    let (status, deleted) = send(
        app.clone(),
        Method::DELETE,
        "/v1/me/reviews/community-mode",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");

    let (status, report) = send(
        app.clone(),
        Method::POST,
        "/v1/modes/community-mode/reports",
        Some(&token),
        Some(json!({ "reason": "Нарушает правила" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");

    let (status, reports) = send(
        app.clone(),
        Method::GET,
        "/v1/admin/reports?status=open",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reports}");
    assert_eq!(reports.as_array().unwrap().len(), 1);
    assert_eq!(reports[0]["targetSlug"], "community-mode");
    assert_eq!(reports[0]["reason"], "Нарушает правила");
    let report_id = reports[0]["id"].as_str().expect("report id");

    let (status, closed) = send(
        app.clone(),
        Method::POST,
        &format!("/v1/admin/reports/{report_id}/close"),
        Some(&admin_token),
        Some(json!({ "note": "Проверено" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{closed}");

    let (status, reports) = send(
        app.clone(),
        Method::GET,
        "/v1/admin/reports?status=open",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reports}");
    assert_eq!(reports, json!([]));

    let (status, history) = send(
        app.clone(),
        Method::GET,
        "/v1/admin/moderation/history?limit=20",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert!(history
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["action"] == "report_close"));

    let (status, empty) = send(
        app,
        Method::GET,
        "/v1/modes/community-mode/reviews",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["count"], 0);

    let _ = (user_id, author_token);
}

#[sqlx::test]
async fn players_cannot_create_modes(pool: PgPool) {
    prepare(&pool).await;
    let (_, token) = create_user_token(&pool, "player", "player").await;
    let (state, _dir) = test_state(pool);
    let app = app(state);

    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/publisher/modes",
        Some(&token),
        Some(json!({
            "slug": "player-mode",
            "title": "Player Mode"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, body) = send(app, Method::GET, "/v1/publisher/modes", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}
