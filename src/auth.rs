use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, FromRow)]
struct AuthRow {
    id: Uuid,
    username: String,
    display_name: String,
    role: String,
}

impl From<AuthRow> for AuthUser {
    fn from(row: AuthRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            display_name: row.display_name,
            role: row.role,
        }
    }
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn require_user(pool: &PgPool, headers: &HeaderMap) -> Result<AuthUser, ApiError> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::Unauthorized("Нужен Bearer-токен".to_string()))?;

    let hash = hash_token(token);
    let row = sqlx::query_as::<_, AuthRow>(
        r#"
        SELECT u.id, u.username, u.display_name, u.role
        FROM api_tokens t
        JOIN users u ON u.id = t.user_id
        WHERE t.token_hash = $1
        "#,
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    let Some(user) = row else {
        return Err(ApiError::Unauthorized("Неверный токен".to_string()));
    };

    let _ = sqlx::query("UPDATE api_tokens SET last_used_at = now() WHERE token_hash = $1")
        .bind(&hash)
        .execute(pool)
        .await;

    Ok(user.into())
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
