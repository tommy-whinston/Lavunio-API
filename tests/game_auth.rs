use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use http_body_util::BodyExt;
use image::{ImageEncoder, Rgba, RgbaImage};
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
        .header(header::CONTENT_TYPE, "image/png")
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

fn png_bytes(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba(color));
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

fn textures_payload(profile: &Value) -> Value {
    let value = profile["properties"]
        .as_array()
        .and_then(|properties| properties.first())
        .and_then(|property| property.get("value"))
        .and_then(Value::as_str)
        .expect("textures property");
    let decoded = BASE64.decode(value).expect("base64 textures");
    serde_json::from_slice(&decoded).expect("textures json")
}

async fn register(pool: PgPool) -> (AppState, tempfile::TempDir, Router, String, String) {
    let (state, dir) = test_state(pool);
    let app = app(state.clone());
    let password = "game password 123";
    let (status, body) = send(
        app.clone(),
        Method::POST,
        "/v1/auth/register",
        None,
        Some(json!({
            "email": "gamer@example.com",
            "username": "gamer",
            "password": password
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["tokens"]["accessToken"]
        .as_str()
        .expect("access token")
        .to_string();
    (state, dir, app, token, password.to_string())
}

#[sqlx::test]
async fn skin_and_cape_are_published_in_profile(pool: PgPool) {
    let (_state, _dir, app, token, _password) = register(pool).await;

    let (status, me) = send(app.clone(), Method::GET, "/v1/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let uuid = me["minecraftUuid"]
        .as_str()
        .expect("minecraft uuid")
        .to_string();
    assert_eq!(me["skinModel"], "classic");

    let (status, updated) = send_bytes(
        app.clone(),
        Method::PUT,
        "/v1/me/skin?model=slim",
        &token,
        Bytes::from(png_bytes(64, 64, [229, 50, 43, 255])),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["skinModel"], "slim");
    let skin_url = updated["skinUrl"].as_str().expect("skin url").to_string();
    assert!(skin_url.contains("/uploads/skins/"));
    assert!(skin_url.ends_with(".png"));

    let (status, profile) = send(
        app.clone(),
        Method::GET,
        &format!("/sessionserver/session/minecraft/profile/{uuid}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    assert_eq!(profile["name"], "gamer");
    let textures = textures_payload(&profile);
    assert_eq!(textures["textures"]["SKIN"]["url"], skin_url);
    assert_eq!(textures["textures"]["SKIN"]["metadata"]["model"], "slim");

    let (status, cape) = send_bytes(
        app.clone(),
        Method::PUT,
        "/v1/me/cape",
        &token,
        Bytes::from(png_bytes(64, 32, [26, 155, 239, 255])),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cape}");
    assert!(cape["capeUrl"]
        .as_str()
        .unwrap()
        .contains("/uploads/skins/"));

    let (status, profile) = send(
        app.clone(),
        Method::GET,
        &format!("/sessionserver/session/minecraft/profile/{uuid}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    let textures = textures_payload(&profile);
    assert_eq!(textures["textures"]["CAPE"]["url"], cape["capeUrl"]);

    let (status, deleted) = send(app, Method::DELETE, "/v1/me/skin", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert!(deleted["skinUrl"].is_null());
}

#[sqlx::test]
async fn authlib_login_refresh_join_flow(pool: PgPool) {
    let (_state, _dir, app, account_token, password) = register(pool).await;

    let (status, login) = send(
        app.clone(),
        Method::POST,
        "/authserver/authenticate",
        None,
        Some(json!({
            "username": "gamer@example.com",
            "password": password,
            "requestUser": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let access = login["accessToken"]
        .as_str()
        .expect("game access")
        .to_string();
    let client = login["clientToken"]
        .as_str()
        .expect("client token")
        .to_string();
    let profile_id = login["selectedProfile"]["id"]
        .as_str()
        .expect("profile id")
        .to_string();
    assert_eq!(profile_id.len(), 32);
    assert_eq!(login["selectedProfile"]["name"], "gamer");
    assert!(login["user"].is_object());

    let (status, _) = send(
        app.clone(),
        Method::POST,
        "/authserver/validate",
        None,
        Some(json!({ "accessToken": access, "clientToken": client })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, refreshed) = send(
        app.clone(),
        Method::POST,
        "/authserver/refresh",
        None,
        Some(json!({
            "accessToken": access,
            "clientToken": client,
            "requestUser": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refreshed}");
    let new_access = refreshed["accessToken"]
        .as_str()
        .expect("new game access")
        .to_string();
    assert_ne!(new_access, access);

    let (status, _) = send(
        app.clone(),
        Method::POST,
        "/authserver/validate",
        None,
        Some(json!({ "accessToken": access })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, joined) = send(
        app.clone(),
        Method::POST,
        "/sessionserver/session/minecraft/join",
        None,
        Some(json!({
            "accessToken": new_access,
            "selectedProfile": profile_id,
            "serverId": "test-server"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{joined}");

    let (status, has_joined) = send(
        app.clone(),
        Method::GET,
        "/sessionserver/session/minecraft/hasJoined?username=gamer&serverId=test-server",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{has_joined}");
    assert_eq!(has_joined["name"], "gamer");

    let (status, _) = send(
        app.clone(),
        Method::GET,
        "/sessionserver/session/minecraft/hasJoined?username=gamer&serverId=test-server",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(
        app.clone(),
        Method::POST,
        "/authserver/invalidate",
        None,
        Some(json!({ "accessToken": new_access })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, game_session) = send(
        app.clone(),
        Method::POST,
        "/v1/game/session",
        Some(&account_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{game_session}");
    assert_eq!(game_session["profile"]["id"], profile_id);
    assert_eq!(game_session["profile"]["name"], "gamer");

    let game_token = game_session["accessToken"]
        .as_str()
        .expect("game access token")
        .to_string();
    let (status, logged_out) = send(
        app.clone(),
        Method::POST,
        "/v1/game/logout",
        Some(&account_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{logged_out}");

    let (status, _) = send(
        app,
        Method::POST,
        "/authserver/validate",
        None,
        Some(json!({ "accessToken": game_token })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
