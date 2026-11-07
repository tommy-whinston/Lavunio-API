use axum::extract::State;
use axum::http::{header::AUTHORIZATION, HeaderMap};
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    AccountUser, AuthResponse, ForgotPasswordRequest, ForgotPasswordResponse, IdentitySummary,
    LoginRequest, LogoutRequest, RefreshRequest, RegisterRequest, RegisterResponse,
    ResetPasswordRequest, SimpleStatus, VerificationResponse, VerifyEmailRequest,
};
use crate::AppState;

const MAX_TTL_SECS: u64 = 365 * 24 * 60 * 60;

#[derive(Debug, FromRow)]
struct LoginRow {
    id: Uuid,
    username: String,
    display_name: String,
    email: Option<String>,
    email_verified_at: Option<DateTime<Utc>>,
    avatar_url: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
    password_hash: Option<String>,
}

impl LoginRow {
    fn account(&self) -> AccountUser {
        AccountUser {
            id: self.id,
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            email: self.email.clone(),
            email_verified: self.email_verified_at.is_some(),
            avatar_url: self.avatar_url.clone(),
            role: self.role.clone(),
            created_at: self.created_at,
        }
    }
}

fn ttl(value: u64) -> Duration {
    Duration::seconds(value.min(MAX_TTL_SECS) as i64)
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    let email = auth::normalize_email(&body.email);
    if !auth::validate_email(&email) {
        return Err(ApiError::BadRequest("Укажите корректный email".to_string()));
    }
    if !auth::validate_password(&body.password) {
        return Err(ApiError::BadRequest(
            "Пароль должен быть от 8 до 128 символов".to_string(),
        ));
    }

    let username = body
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| auth::username_from_email(&email));
    if !auth::validate_username(&username) {
        return Err(ApiError::BadRequest(
            "Имя пользователя: 3–20 символов, латиница, цифры и подчёркивание".to_string(),
        ));
    }

    let display_name = body
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(48).collect::<String>())
        .unwrap_or_else(|| username.clone());

    let email_taken = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM users WHERE lower(email) = $1)",
    )
    .bind(&email)
    .fetch_one(&state.pool)
    .await?;
    if email_taken {
        return Err(ApiError::Conflict("Email уже зарегистрирован".to_string()));
    }

    let username_taken = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM users WHERE lower(username) = lower($1))",
    )
    .bind(&username)
    .fetch_one(&state.pool)
    .await?;
    if username_taken {
        return Err(ApiError::Conflict(
            "Имя пользователя уже занято".to_string(),
        ));
    }

    let password_hash = auth::hash_password(body.password.clone()).await?;
    let verification_token = auth::random_token();
    let user_id = Uuid::new_v4();
    let expires_at = Utc::now() + ttl(state.config.email_token_ttl_secs);

    let mut tx = state.pool.begin().await?;
    let inserted = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
        INSERT INTO users (id, username, display_name, email, password_hash, role)
        VALUES ($1, $2, $3, $4, $5, 'player')
        RETURNING created_at
        "#,
    )
    .bind(user_id)
    .bind(&username)
    .bind(&display_name)
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&mut *tx)
    .await;

    let created_at = match inserted {
        Ok(created_at) => created_at,
        Err(error) => {
            if let sqlx::Error::Database(database_error) = &error {
                if database_error.code().as_deref() == Some("23505") {
                    return Err(ApiError::Conflict(
                        "Email или имя пользователя уже заняты".to_string(),
                    ));
                }
            }
            return Err(error.into());
        }
    };

    sqlx::query(
        r#"
        INSERT INTO identities (id, user_id, provider, external_id, display_name)
        VALUES ($1, $2, 'lavunio', $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(user_id.to_string())
    .bind(&display_name)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO email_verification_tokens (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(auth::hash_token(&verification_token))
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let (user_agent, ip) = auth::session_metadata(&headers);
    let tokens = auth::create_session(
        &state.pool,
        user_id,
        user_agent.as_deref(),
        ip.as_deref(),
        state.config.access_token_ttl_secs,
        state.config.refresh_token_ttl_secs,
    )
    .await?;

    if state.config.dev_mode {
        tracing::info!(
            email = %email,
            token = %verification_token,
            "email verification token issued (dev mode)"
        );
    }

    Ok(Json(RegisterResponse {
        user: AccountUser {
            id: user_id,
            username,
            display_name,
            email: Some(email),
            email_verified: false,
            avatar_url: None,
            role: "player".to_string(),
            created_at,
        },
        tokens,
        verification_token: state.config.dev_mode.then_some(verification_token),
    }))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = auth::normalize_email(&body.email);
    let row = sqlx::query_as::<_, LoginRow>(
        r#"
        SELECT id, username, display_name, email, email_verified_at, avatar_url, role,
               created_at, password_hash
        FROM users
        WHERE lower(email) = $1
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;

    let (user, hash, has_password) = match row {
        Some(row) => {
            let hash = row
                .password_hash
                .clone()
                .unwrap_or_else(|| auth::dummy_password_hash().to_string());
            let has_password = row.password_hash.is_some();
            (Some(row), hash, has_password)
        }
        None => (None, auth::dummy_password_hash().to_string(), false),
    };

    let password_ok = auth::verify_password(body.password, hash).await;
    if !password_ok || !has_password {
        return Err(ApiError::Unauthorized(
            "Неверный email или пароль".to_string(),
        ));
    }

    let user =
        user.ok_or_else(|| ApiError::Unauthorized("Неверный email или пароль".to_string()))?;
    let (user_agent, ip) = auth::session_metadata(&headers);
    let tokens = auth::create_session(
        &state.pool,
        user.id,
        user_agent.as_deref(),
        ip.as_deref(),
        state.config.access_token_ttl_secs,
        state.config.refresh_token_ttl_secs,
    )
    .await?;

    Ok(Json(AuthResponse {
        user: user.account(),
        tokens,
    }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let (tokens, user) = auth::rotate_session(
        &state.pool,
        body.refresh_token.trim(),
        state.config.access_token_ttl_secs,
        state.config.refresh_token_ttl_secs,
    )
    .await?;

    Ok(Json(AuthResponse {
        user: user.account(),
        tokens,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<LogoutRequest>>,
) -> Result<Json<SimpleStatus>, ApiError> {
    if let Some(token) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        auth::revoke_by_access_token(&state.pool, token).await?;
    }

    if let Some(refresh_token) = body
        .and_then(|body| body.0.refresh_token)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        auth::revoke_by_refresh_token(&state.pool, &refresh_token).await?;
    }

    Ok(Json(SimpleStatus {
        ok: true,
        status: "logged_out".to_string(),
    }))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountUser>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    Ok(Json(user.account()))
}

pub async fn identities(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IdentitySummary>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let identities = sqlx::query_as::<_, IdentitySummary>(
        r#"
        SELECT id, provider, external_id, display_name, avatar_url, created_at
        FROM identities
        WHERE user_id = $1
        ORDER BY created_at
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(identities))
}

pub async fn verify_email(
    State(state): State<AppState>,
    Json(body): Json<VerifyEmailRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let token = body.token.trim();
    if token.is_empty() {
        return Err(ApiError::BadRequest("Нужен токен".to_string()));
    }

    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        SELECT id, user_id
        FROM email_verification_tokens
        WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
        "#,
    )
    .bind(auth::hash_token(token))
    .fetch_optional(&state.pool)
    .await?;

    let Some((token_id, user_id)) = row else {
        return Err(ApiError::BadRequest(
            "Ссылка подтверждения недействительна или истекла".to_string(),
        ));
    };

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE email_verification_tokens SET used_at = now() WHERE id = $1")
        .bind(token_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()), updated_at = now() WHERE id = $1",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(SimpleStatus {
        ok: true,
        status: "email_verified".to_string(),
    }))
}

pub async fn resend_verification(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VerificationResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    if user.email_verified {
        return Ok(Json(VerificationResponse {
            ok: true,
            verification_token: None,
        }));
    }
    let email = user
        .email
        .clone()
        .ok_or_else(|| ApiError::BadRequest("У аккаунта нет email".to_string()))?;

    let token = auth::random_token();
    let expires_at = Utc::now() + ttl(state.config.email_token_ttl_secs);
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE email_verification_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_verification_tokens (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(auth::hash_token(&token))
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    if state.config.dev_mode {
        tracing::info!(
            email = %email,
            token = %token,
            "email verification token reissued (dev mode)"
        );
    }

    Ok(Json(VerificationResponse {
        ok: true,
        verification_token: state.config.dev_mode.then_some(token),
    }))
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordRequest>,
) -> Result<Json<ForgotPasswordResponse>, ApiError> {
    let email = auth::normalize_email(&body.email);
    let user =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, email FROM users WHERE lower(email) = $1")
            .bind(&email)
            .fetch_optional(&state.pool)
            .await?;

    if let Some((user_id, email)) = user {
        let token = auth::random_token();
        let expires_at = Utc::now() + ttl(state.config.reset_token_ttl_secs);
        let mut tx = state.pool.begin().await?;
        sqlx::query(
            "UPDATE password_reset_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(auth::hash_token(&token))
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        if state.config.dev_mode {
            tracing::info!(
                email = %email,
                token = %token,
                "password reset token issued (dev mode)"
            );
        }

        return Ok(Json(ForgotPasswordResponse {
            ok: true,
            reset_token: state.config.dev_mode.then_some(token),
        }));
    }

    Ok(Json(ForgotPasswordResponse {
        ok: true,
        reset_token: None,
    }))
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let token = body.token.trim();
    if token.is_empty() {
        return Err(ApiError::BadRequest("Нужен токен".to_string()));
    }
    if !auth::validate_password(&body.password) {
        return Err(ApiError::BadRequest(
            "Пароль должен быть от 8 до 128 символов".to_string(),
        ));
    }

    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        SELECT id, user_id
        FROM password_reset_tokens
        WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
        "#,
    )
    .bind(auth::hash_token(token))
    .fetch_optional(&state.pool)
    .await?;

    let Some((_, user_id)) = row else {
        return Err(ApiError::BadRequest(
            "Ссылка сброса пароля недействительна или истекла".to_string(),
        ));
    };

    let password_hash = auth::hash_password(body.password).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
        .bind(user_id)
        .bind(&password_hash)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE password_reset_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Json(SimpleStatus {
        ok: true,
        status: "password_reset".to_string(),
    }))
}
