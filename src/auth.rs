use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use axum::http::header::{AUTHORIZATION, USER_AGENT};
use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{AccountUser, AuthTokens};

const MAX_TOKEN_TTL_SECS: u64 = 365 * 24 * 60 * 60;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub avatar_url: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub minecraft_uuid: Option<Uuid>,
    pub skin_url: Option<String>,
    pub skin_model: String,
    pub cape_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: AuthUser,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct AuthRow {
    session_id: Option<Uuid>,
    id: Uuid,
    username: String,
    display_name: String,
    email: Option<String>,
    email_verified_at: Option<DateTime<Utc>>,
    avatar_url: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
    minecraft_uuid: Option<Uuid>,
    skin_url: Option<String>,
    skin_model: String,
    cape_url: Option<String>,
}

impl From<AuthRow> for AuthSession {
    fn from(row: AuthRow) -> Self {
        Self {
            session_id: row.session_id,
            user: AuthUser {
                id: row.id,
                username: row.username,
                display_name: row.display_name,
                email: row.email,
                email_verified: row.email_verified_at.is_some(),
                avatar_url: row.avatar_url,
                role: row.role,
                created_at: row.created_at,
                minecraft_uuid: row.minecraft_uuid,
                skin_url: row.skin_url,
                skin_model: row.skin_model,
                cape_url: row.cape_url,
            },
        }
    }
}

impl AuthUser {
    pub fn account(&self) -> AccountUser {
        AccountUser {
            id: self.id,
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            email: self.email.clone(),
            email_verified: self.email_verified,
            avatar_url: self.avatar_url.clone(),
            role: self.role.clone(),
            created_at: self.created_at,
            minecraft_uuid: self.minecraft_uuid.map(|value| value.simple().to_string()),
            skin_url: self.skin_url.clone(),
            skin_model: self.skin_model.clone(),
            cape_url: self.cape_url.clone(),
        }
    }
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn random_token() -> String {
    format!("lv_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn normalize_email(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn validate_email(value: &str) -> bool {
    let value = value.trim();
    if !(5..=254).contains(&value.len()) || value.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = value.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    parts.next().is_none()
        && !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

pub fn validate_username(value: &str) -> bool {
    let length = value.chars().count();
    (3..=20).contains(&length)
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| ch.is_ascii_alphanumeric() || (index > 0 && ch == '_'))
}

pub fn validate_password(value: &str) -> bool {
    (8..=128).contains(&value.chars().count()) && value.chars().any(|ch| !ch.is_whitespace())
}

pub fn username_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or_default();
    let mut name: String = local
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    name = name.trim_start_matches('_').to_string();
    if name.len() < 3 {
        name = format!("player{name}");
    }
    name.truncate(20);
    name
}

fn hash_password_sync(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| error.to_string())
}

fn verify_password_sync(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

pub async fn hash_password(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || hash_password_sync(&password))
        .await
        .map_err(|error| ApiError::Internal(format!("Не удалось захешировать пароль: {error}")))?
        .map_err(|error| ApiError::Internal(format!("Не удалось захешировать пароль: {error}")))
}

pub async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password_sync(&password, &hash))
        .await
        .unwrap_or(false)
}

pub fn dummy_password_hash() -> &'static str {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password_sync("lavunio-invalid-password").unwrap_or_else(|_| {
            "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$6aGq0G6mY7dQxk8Zx7dQhQ".to_string()
        })
    })
}

pub async fn require_user(pool: &PgPool, headers: &HeaderMap) -> Result<AuthUser, ApiError> {
    Ok(require_session(pool, headers).await?.user)
}

pub async fn require_session(pool: &PgPool, headers: &HeaderMap) -> Result<AuthSession, ApiError> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::Unauthorized("Нужен Bearer-токен".to_string()))?;

    let hash = hash_token(token);

    let api_user = sqlx::query_as::<_, AuthRow>(
        r#"
        SELECT NULL::uuid AS session_id,
               u.id, u.username, u.display_name, u.email, u.email_verified_at,
               u.avatar_url, u.role, u.created_at, u.minecraft_uuid, u.skin_url,
               u.skin_model, u.cape_url
        FROM api_tokens t
        JOIN users u ON u.id = t.user_id
        WHERE t.token_hash = $1
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = api_user {
        let _ = sqlx::query("UPDATE api_tokens SET last_used_at = now() WHERE token_hash = $1")
            .bind(&hash)
            .execute(pool)
            .await;
        return Ok(row.into());
    }

    let session_user = sqlx::query_as::<_, AuthRow>(
        r#"
        SELECT s.id AS session_id,
               u.id, u.username, u.display_name, u.email, u.email_verified_at,
               u.avatar_url, u.role, u.created_at, u.minecraft_uuid, u.skin_url,
               u.skin_model, u.cape_url
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.access_hash = $1
          AND s.revoked_at IS NULL
          AND s.access_expires_at > now()
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    let Some(row) = session_user else {
        return Err(ApiError::Unauthorized("Неверный токен".to_string()));
    };

    if let Some(session_id) = row.session_id {
        let _ = sqlx::query("UPDATE sessions SET last_used_at = now() WHERE id = $1")
            .bind(session_id)
            .execute(pool)
            .await;
    }

    Ok(row.into())
}

pub async fn account_by_id(pool: &PgPool, user_id: Uuid) -> Result<AccountUser, ApiError> {
    let row = sqlx::query_as::<_, AuthRow>(
        r#"
        SELECT NULL::uuid AS session_id,
               u.id, u.username, u.display_name, u.email, u.email_verified_at,
               u.avatar_url, u.role, u.created_at, u.minecraft_uuid, u.skin_url,
               u.skin_model, u.cape_url
        FROM users u
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(AuthSession::from(row).user.account())
}

pub async fn require_admin(pool: &PgPool, headers: &HeaderMap) -> Result<AuthUser, ApiError> {
    let user = require_user(pool, headers).await?;
    if user.role != "admin" {
        return Err(ApiError::Forbidden(
            "Нужны права администратора".to_string(),
        ));
    }
    Ok(user)
}

pub fn session_metadata(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.chars().take(512).collect::<String>());
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    (user_agent, ip)
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    user_agent: Option<&str>,
    ip: Option<&str>,
    access_ttl_secs: u64,
    refresh_ttl_secs: u64,
) -> Result<AuthTokens, ApiError> {
    let access_token = random_token();
    let refresh_token = random_token();
    let now = Utc::now();
    let access_expires_at = now + Duration::seconds(ttl_secs(access_ttl_secs));
    let refresh_expires_at = now + Duration::seconds(ttl_secs(refresh_ttl_secs));

    sqlx::query(
        r#"
        INSERT INTO sessions
            (id, user_id, access_hash, refresh_hash, access_expires_at, refresh_expires_at,
             user_agent, ip)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(hash_token(&access_token))
    .bind(hash_token(&refresh_token))
    .bind(access_expires_at)
    .bind(refresh_expires_at)
    .bind(user_agent)
    .bind(ip)
    .execute(pool)
    .await?;

    Ok(AuthTokens {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: access_ttl_secs as i64,
        refresh_expires_in: refresh_ttl_secs as i64,
    })
}

pub async fn rotate_session(
    pool: &PgPool,
    refresh_token: &str,
    access_ttl_secs: u64,
    refresh_ttl_secs: u64,
) -> Result<(AuthTokens, AuthUser), ApiError> {
    let hash = hash_token(refresh_token);
    let row = sqlx::query_as::<_, AuthRow>(
        r#"
        SELECT s.id AS session_id,
               u.id, u.username, u.display_name, u.email, u.email_verified_at,
               u.avatar_url, u.role, u.created_at, u.minecraft_uuid, u.skin_url,
               u.skin_model, u.cape_url
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.refresh_hash = $1
          AND s.revoked_at IS NULL
          AND s.refresh_expires_at > now()
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Err(ApiError::Unauthorized(
            "Сессия истекла, войдите заново".to_string(),
        ));
    };

    let session_id = row
        .session_id
        .ok_or_else(|| ApiError::Internal("У сессии нет идентификатора".to_string()))?;
    let user: AuthSession = row.into();

    let access_token = random_token();
    let new_refresh_token = random_token();
    let now = Utc::now();
    let access_expires_at = now + Duration::seconds(ttl_secs(access_ttl_secs));
    let refresh_expires_at = now + Duration::seconds(ttl_secs(refresh_ttl_secs));

    let updated = sqlx::query(
        r#"
        UPDATE sessions
        SET access_hash = $2,
            refresh_hash = $3,
            access_expires_at = $4,
            refresh_expires_at = $5,
            last_used_at = now()
        WHERE id = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(session_id)
    .bind(hash_token(&access_token))
    .bind(hash_token(&new_refresh_token))
    .bind(access_expires_at)
    .bind(refresh_expires_at)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::Unauthorized("Сессия уже завершена".to_string()));
    }

    Ok((
        AuthTokens {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: access_ttl_secs as i64,
            refresh_expires_in: refresh_ttl_secs as i64,
        },
        user.user,
    ))
}

pub async fn revoke_session(pool: &PgPool, session_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_by_access_token(pool: &PgPool, token: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE access_hash = $1 AND revoked_at IS NULL",
    )
    .bind(hash_token(token))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke_by_refresh_token(pool: &PgPool, token: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE refresh_hash = $1 AND revoked_at IS NULL",
    )
    .bind(hash_token(token))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke_all_sessions(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn ensure_bootstrap(pool: &PgPool, token: Option<&str>) {
    let Some(token) = token else {
        return;
    };
    let hash = hash_token(token);

    match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM api_tokens WHERE token_hash = $1")
        .bind(&hash)
        .fetch_one(pool)
        .await
    {
        Ok(count) if count > 0 => return,
        Ok(_) => {}
        Err(error) => {
            tracing::warn!("bootstrap token skipped: database unavailable ({error})");
            return;
        }
    }

    let user_id = Uuid::new_v4();
    let user = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (id, username, display_name, role)
        VALUES ($1, 'tm', 'TM', 'admin')
        ON CONFLICT (username) DO UPDATE SET role = 'admin'
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await;

    let user_id = match user {
        Ok(id) => id,
        Err(error) => {
            tracing::warn!("bootstrap user failed: {error}");
            return;
        }
    };

    let result = sqlx::query(
        r#"
        INSERT INTO api_tokens (id, user_id, name, token_hash, scopes)
        VALUES ($1, $2, 'bootstrap', $3, '["publisher","admin"]'::jsonb)
        ON CONFLICT (token_hash) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(&hash)
    .execute(pool)
    .await;

    match result {
        Ok(_) => tracing::info!("bootstrap API token is ready for user tm"),
        Err(error) => tracing::warn!("bootstrap token failed: {error}"),
    }
}

pub async fn create_token_for_user(
    pool: &PgPool,
    username: &str,
    name: &str,
    scopes: &[String],
) -> Result<String, String> {
    let user_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (id, username, display_name, role)
        VALUES ($1, $2, $2, 'author')
        ON CONFLICT (username) DO UPDATE SET display_name = EXCLUDED.display_name
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(username)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("Не удалось создать пользователя: {error}"))?;

    let token = format!("tm_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let hash = hash_token(&token);
    let scopes = serde_json::json!(scopes);

    sqlx::query(
        r#"
        INSERT INTO api_tokens (id, user_id, name, token_hash, scopes)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(name)
    .bind(hash)
    .bind(scopes)
    .execute(pool)
    .await
    .map_err(|error| format!("Не удалось создать токен: {error}"))?;

    Ok(token)
}

fn ttl_secs(value: u64) -> i64 {
    value.min(MAX_TOKEN_TTL_SECS) as i64
}
