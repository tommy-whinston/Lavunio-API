use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{Duration, Utc};
use image::{ImageEncoder, ImageFormat};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{AccountUser, GameProfile, GameSessionResponse};
use crate::rate_limit::Bucket;
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateRequest {
    username: String,
    password: String,
    #[serde(default)]
    client_token: Option<String>,
    #[serde(default)]
    request_user: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    access_token: String,
    #[serde(default)]
    client_token: Option<String>,
    #[serde(default)]
    request_user: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRequest {
    access_token: String,
    #[serde(default)]
    client_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignoutRequest {
    username: String,
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    access_token: String,
    selected_profile: String,
    server_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HasJoinedQuery {
    username: String,
    server_id: String,
    #[serde(default)]
    ip: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkinQuery {
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkinModelRequest {
    model: String,
}

#[derive(Debug, FromRow)]
pub struct GameUserRow {
    id: Uuid,
    username: String,
    minecraft_uuid: Uuid,
    skin_url: Option<String>,
    skin_model: String,
    cape_url: Option<String>,
}

fn ygg_error(status: StatusCode, error: &str, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": error,
            "errorMessage": message,
        })),
    )
        .into_response()
}

fn forbidden(message: &str) -> Response {
    ygg_error(
        StatusCode::FORBIDDEN,
        "ForbiddenOperationException",
        message,
    )
}

fn illegal(message: &str) -> Response {
    ygg_error(StatusCode::BAD_REQUEST, "IllegalArgumentException", message)
}

fn profile_json(user: &GameUserRow) -> Value {
    let mut textures = serde_json::Map::new();
    if let Some(url) = user.skin_url.as_deref().filter(|url| !url.is_empty()) {
        let model = if user.skin_model == "slim" {
            "slim"
        } else {
            "default"
        };
        textures.insert(
            "SKIN".to_string(),
            json!({
                "url": url,
                "metadata": { "model": model }
            }),
        );
    }

    let properties = if textures.is_empty() {
        json!([])
    } else {
        let payload = json!({
            "timestamp": Utc::now().timestamp_millis(),
            "profileId": user.minecraft_uuid.simple().to_string(),
            "profileName": user.username,
            "textures": textures,
        });
        let encoded = BASE64.encode(serde_json::to_vec(&payload).unwrap_or_default());
        json!([{ "name": "textures", "value": encoded }])
    };

    json!({
        "id": user.minecraft_uuid.simple().to_string(),
        "name": user.username,
        "properties": properties,
    })
}

fn profile_stub(user_id: Uuid) -> Value {
    json!({
        "id": user_id.simple().to_string(),
        "properties": [],
    })
}

async fn mint_game_session(
    pool: &PgPool,
    user_id: Uuid,
    client_token: Option<String>,
    ttl_secs: u64,
) -> Result<(String, String), ApiError> {
    let access_token = auth::random_token();
    let client_token = client_token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(auth::random_token);
    let expires_at = Utc::now() + Duration::seconds(ttl_secs.min(365 * 24 * 60 * 60) as i64);

    sqlx::query(
        r#"
        INSERT INTO game_sessions
            (id, user_id, access_hash, client_hash, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(auth::hash_token(&access_token))
    .bind(auth::hash_token(&client_token))
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok((access_token, client_token))
}

fn auth_response(
    access_token: String,
    client_token: String,
    user_id: Uuid,
    username: &str,
    uuid: Uuid,
    request_user: bool,
) -> Value {
    let profile = json!({
        "id": uuid.simple().to_string(),
        "name": username,
    });
    let mut response = json!({
        "accessToken": access_token,
        "clientToken": client_token,
        "availableProfiles": [profile.clone()],
        "selectedProfile": profile,
    });
    if request_user {
        response["user"] = profile_stub(user_id);
    }
    response
}

pub async fn create_game_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GameSessionResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let uuid = user
        .minecraft_uuid
        .ok_or_else(|| ApiError::Internal("У аккаунта нет Minecraft UUID".to_string()))?;
    let ttl = state.config.game_token_ttl_secs;
    let (access_token, client_token) = mint_game_session(&state.pool, user.id, None, ttl).await?;
    let account = auth::account_by_id(&state.pool, user.id).await?;

    Ok(Json(GameSessionResponse {
        access_token,
        client_token,
        expires_in: ttl as i64,
        profile: GameProfile {
            id: uuid.simple().to_string(),
            name: user.username,
        },
        skin_url: account.skin_url,
        skin_model: account.skin_model,
        cape_url: account.cape_url,
    }))
}

pub async fn revoke_game_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::models::SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    sqlx::query(
        "UPDATE game_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(crate::models::SimpleStatus {
        ok: true,
        status: "game_sessions_revoked".to_string(),
    }))
}

pub async fn authenticate(
    State(state): State<AppState>,
    Json(body): Json<AuthenticateRequest>,
) -> Response {
    let username = body.username.trim();
    if let Err(error) = state
        .rate_limiter
        .check(Bucket::Login, &username.to_lowercase())
    {
        return ygg_error(
            StatusCode::TOO_MANY_REQUESTS,
            "ForbiddenOperationException",
            &error.to_string(),
        );
    }
    if username.is_empty() || body.password.is_empty() {
        return illegal("Нужны логин и пароль");
    }

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Uuid)>(
        r#"
        SELECT id, username, password_hash, minecraft_uuid
        FROM users
        WHERE lower(email) = lower($1) OR lower(username) = lower($1)
        "#,
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await;

    let row = match row {
        Ok(row) => row,
        Err(error) => {
            tracing::error!("authlib authenticate query failed: {error}");
            return ygg_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "Внутренняя ошибка сервера",
            );
        }
    };

    let Some((user_id, username, password_hash, minecraft_uuid)) = row else {
        let _ = auth::verify_password(
            body.password.clone(),
            auth::dummy_password_hash().to_string(),
        )
        .await;
        return forbidden("Неверный логин или пароль");
    };

    let Some(password_hash) = password_hash else {
        return forbidden("У аккаунта не задан пароль");
    };
    if !auth::verify_password(body.password, password_hash).await {
        return forbidden("Неверный логин или пароль");
    }

    match mint_game_session(
        &state.pool,
        user_id,
        body.client_token,
        state.config.game_token_ttl_secs,
    )
    .await
    {
        Ok((access_token, client_token)) => {
            let response = auth_response(
                access_token,
                client_token,
                user_id,
                &username,
                minecraft_uuid,
                body.request_user,
            );
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => {
            tracing::error!("authlib session mint failed: {error}");
            ygg_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "Не удалось создать игровую сессию",
            )
        }
    }
}

pub async fn refresh(State(state): State<AppState>, Json(body): Json<RefreshRequest>) -> Response {
    let row = sqlx::query_as::<_, (Uuid, Uuid, Option<String>, String, Uuid)>(
        r#"
        SELECT gs.id, u.id, gs.client_hash, u.username, u.minecraft_uuid
        FROM game_sessions gs
        JOIN users u ON u.id = gs.user_id
        WHERE gs.access_hash = $1
          AND gs.revoked_at IS NULL
          AND gs.expires_at > now()
        "#,
    )
    .bind(auth::hash_token(&body.access_token))
    .fetch_optional(&state.pool)
    .await;

    let row = match row {
        Ok(row) => row,
        Err(error) => {
            tracing::error!("authlib refresh query failed: {error}");
            return ygg_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "Внутренняя ошибка сервера",
            );
        }
    };

    let Some((session_id, user_id, client_hash, username, minecraft_uuid)) = row else {
        return forbidden("Сессия истекла, войдите заново");
    };
    if let (Some(expected), Some(actual)) = (client_hash.as_deref(), body.client_token.as_deref()) {
        if auth::hash_token(actual) != expected {
            return forbidden("Неверный clientToken");
        }
    }

    let access_token = auth::random_token();
    let client_token = body
        .client_token
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(auth::random_token);
    let expires_at = Utc::now()
        + Duration::seconds(state.config.game_token_ttl_secs.min(365 * 24 * 60 * 60) as i64);

    let updated = sqlx::query(
        r#"
        UPDATE game_sessions
        SET access_hash = $2,
            client_hash = $3,
            expires_at = $4,
            last_used_at = now()
        WHERE id = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(session_id)
    .bind(auth::hash_token(&access_token))
    .bind(auth::hash_token(&client_token))
    .bind(expires_at)
    .execute(&state.pool)
    .await;

    if let Err(error) = updated {
        tracing::error!("authlib refresh update failed: {error}");
        return ygg_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalServerError",
            "Не удалось обновить сессию",
        );
    }

    let response = auth_response(
        access_token,
        client_token,
        user_id,
        &username,
        minecraft_uuid,
        body.request_user,
    );
    (StatusCode::OK, Json(response)).into_response()
}

pub async fn validate(State(state): State<AppState>, Json(body): Json<TokenRequest>) -> Response {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        r#"
        SELECT client_hash
        FROM game_sessions
        WHERE access_hash = $1
          AND revoked_at IS NULL
          AND expires_at > now()
        "#,
    )
    .bind(auth::hash_token(&body.access_token))
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some((client_hash,))) => {
            if let (Some(expected), Some(actual)) =
                (client_hash.as_deref(), body.client_token.as_deref())
            {
                if auth::hash_token(actual) != expected {
                    return forbidden("Неверный clientToken");
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(None) => forbidden("Сессия недействительна"),
        Err(error) => {
            tracing::error!("authlib validate failed: {error}");
            ygg_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "Внутренняя ошибка сервера",
            )
        }
    }
}

pub async fn invalidate(State(state): State<AppState>, Json(body): Json<TokenRequest>) -> Response {
    let _ = sqlx::query(
        "UPDATE game_sessions SET revoked_at = now() WHERE access_hash = $1 AND revoked_at IS NULL",
    )
    .bind(auth::hash_token(&body.access_token))
    .execute(&state.pool)
    .await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn signout(State(state): State<AppState>, Json(body): Json<SignoutRequest>) -> Response {
    if let Err(error) = state
        .rate_limiter
        .check(Bucket::Login, &body.username.trim().to_lowercase())
    {
        return ygg_error(
            StatusCode::TOO_MANY_REQUESTS,
            "ForbiddenOperationException",
            &error.to_string(),
        );
    }
    let row = sqlx::query_as::<_, (Uuid, Option<String>)>(
        r#"
        SELECT id, password_hash
        FROM users
        WHERE lower(email) = lower($1) OR lower(username) = lower($1)
        "#,
    )
    .bind(body.username.trim())
    .fetch_optional(&state.pool)
    .await;

    let Ok(Some((user_id, Some(password_hash)))) = row else {
        return forbidden("Неверный логин или пароль");
    };
    if !auth::verify_password(body.password, password_hash).await {
        return forbidden("Неверный логин или пароль");
    }

    let _ = sqlx::query(
        "UPDATE game_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn profile(State(state): State<AppState>, Path(uuid): Path<String>) -> Response {
    let Ok(uuid) = Uuid::parse_str(uuid.trim()) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    let row = sqlx::query_as::<_, GameUserRow>(
        r#"
        SELECT id, username, minecraft_uuid, skin_url, skin_model, cape_url
        FROM users
        WHERE minecraft_uuid = $1
        "#,
    )
    .bind(uuid)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(user)) => Json(profile_json(&user)).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            tracing::error!("authlib profile failed: {error}");
            ygg_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "Внутренняя ошибка сервера",
            )
        }
    }
}

pub async fn join(State(state): State<AppState>, Json(body): Json<JoinRequest>) -> Response {
    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        SELECT u.id, u.minecraft_uuid
        FROM game_sessions gs
        JOIN users u ON u.id = gs.user_id
        WHERE gs.access_hash = $1
          AND gs.revoked_at IS NULL
          AND gs.expires_at > now()
        "#,
    )
    .bind(auth::hash_token(&body.access_token))
    .fetch_optional(&state.pool)
    .await;

    let Ok(Some((user_id, minecraft_uuid))) = row else {
        return forbidden("Сессия недействительна");
    };
    let Ok(selected_profile) = Uuid::parse_str(body.selected_profile.trim()) else {
        return illegal("Некорректный selectedProfile");
    };
    if selected_profile != minecraft_uuid {
        return forbidden("Профиль не совпадает с сессией");
    }
    if body.server_id.trim().is_empty() {
        return illegal("Нужен serverId");
    }

    let _ = sqlx::query("DELETE FROM game_joins WHERE created_at < now() - interval '2 minutes'")
        .execute(&state.pool)
        .await;
    let _ = sqlx::query(
        r#"
        INSERT INTO game_joins (server_id, user_id)
        VALUES ($1, $2)
        ON CONFLICT (server_id, user_id) DO UPDATE SET created_at = now()
        "#,
    )
    .bind(body.server_id.trim())
    .bind(user_id)
    .execute(&state.pool)
    .await;

    StatusCode::NO_CONTENT.into_response()
}

pub async fn has_joined(
    State(state): State<AppState>,
    Query(query): Query<HasJoinedQuery>,
) -> Response {
    let _ = query.ip;
    let row = sqlx::query_as::<_, GameUserRow>(
        r#"
        SELECT id, username, minecraft_uuid, skin_url, skin_model, cape_url
        FROM users
        WHERE lower(username) = lower($1)
        "#,
    )
    .bind(query.username.trim())
    .fetch_optional(&state.pool)
    .await;

    let Ok(Some(user)) = row else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let deleted = sqlx::query(
        "DELETE FROM game_joins WHERE server_id = $1 AND user_id = $2 AND created_at > now() - interval '2 minutes'",
    )
    .bind(query.server_id.trim())
    .bind(user.id)
    .execute(&state.pool)
    .await;

    match deleted {
        Ok(result) if result.rows_affected() > 0 => Json(profile_json(&user)).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            tracing::error!("authlib hasJoined failed: {error}");
            ygg_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "Внутренняя ошибка сервера",
            )
        }
    }
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

fn sanitize_texture(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let Some((width, height)) = png_dimensions(bytes) else {
        return Err("Файл должен быть PNG".to_string());
    };
    if !matches!((width, height), (64, 64) | (64, 32)) {
        return Err("Скин должен быть 64×64 или 64×32".to_string());
    }

    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|_| "Не удалось прочитать PNG".to_string())?;
    if image.width() != width || image.height() != height {
        return Err("Размер PNG не совпадает с заголовком".to_string());
    }

    let rgba = image.to_rgba8();

    let mut output = Vec::new();
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| "Не удалось сохранить PNG".to_string())?;
    Ok(output)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub async fn upload_skin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SkinQuery>,
    bytes: Bytes,
) -> Result<Json<AccountUser>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let model = query
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("classic");
    if !matches!(model, "classic" | "slim") {
        return Err(ApiError::BadRequest("model: classic или slim".to_string()));
    }
    let Some((width, height)) = png_dimensions(&bytes) else {
        return Err(ApiError::BadRequest(
            "Скин должен быть PNG-файлом".to_string(),
        ));
    };
    if !matches!((width, height), (64, 64) | (64, 32)) {
        return Err(ApiError::BadRequest(
            "Скин должен быть 64×64 или 64×32".to_string(),
        ));
    }
    let sanitized = sanitize_texture(&bytes).map_err(ApiError::BadRequest)?;
    let hash = sha256_hex(&sanitized);
    let key = format!("skins/{hash}.png");
    state
        .storage
        .put_bytes(&key, &sanitized, Some("image/png"))
        .await
        .map_err(ApiError::Internal)?;
    let url = state.storage.public_url(&key);
    sqlx::query(
        "UPDATE users SET skin_url = $2, skin_model = $3, skin_updated_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(user.id)
    .bind(&url)
    .bind(model)
    .execute(&state.pool)
    .await?;

    Ok(Json(auth::account_by_id(&state.pool, user.id).await?))
}

pub async fn update_skin_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateSkinModelRequest>,
) -> Result<Json<AccountUser>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let model = body.model.trim().to_lowercase();
    if !matches!(model.as_str(), "classic" | "slim") {
        return Err(ApiError::BadRequest("model: classic или slim".to_string()));
    }
    sqlx::query("UPDATE users SET skin_model = $2, updated_at = now() WHERE id = $1")
        .bind(user.id)
        .bind(&model)
        .execute(&state.pool)
        .await?;
    Ok(Json(auth::account_by_id(&state.pool, user.id).await?))
}

pub async fn delete_skin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountUser>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    sqlx::query(
        "UPDATE users SET skin_url = NULL, skin_updated_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(auth::account_by_id(&state.pool, user.id).await?))
}
