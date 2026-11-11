use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{ChatMessageItem, ChatSendRequest};
use crate::AppState;

async fn messages(
    pool: &sqlx::PgPool,
    sender: Uuid,
    recipient: Uuid,
) -> Result<Vec<ChatMessageItem>, ApiError> {
    let items = sqlx::query_as::<_, ChatMessageItem>(
        r#"
        SELECT * FROM (
            SELECT m.id, m.sender_id, u.username AS sender_username,
                   u.display_name AS sender_display_name, m.body, m.created_at
            FROM chat_messages m
            JOIN users u ON u.id = m.sender_id
            WHERE m.team_id IS NULL
              AND ((m.sender_id = $1 AND m.recipient_id = $2)
                OR (m.sender_id = $2 AND m.recipient_id = $1))
            ORDER BY m.created_at DESC
            LIMIT 100
        ) messages
        ORDER BY created_at
        "#,
    )
    .bind(sender)
    .bind(recipient)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

pub async fn direct(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Result<Json<Vec<ChatMessageItem>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let other =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE lower(username) = lower($1)")
            .bind(username.trim())
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("Игрок не найден".to_string()))?;
    Ok(Json(messages(&state.pool, user.id, other).await?))
}

pub async fn send_direct(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
    Json(body): Json<ChatSendRequest>,
) -> Result<Json<ChatMessageItem>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let other =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE lower(username) = lower($1)")
            .bind(username.trim())
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("Игрок не найден".to_string()))?;
    if other == user.id {
        return Err(ApiError::BadRequest(
            "Нельзя писать самому себе".to_string(),
        ));
    }
    let text = body.body.trim();
    if text.is_empty() || text.chars().count() > 2000 {
        return Err(ApiError::BadRequest(
            "Сообщение: от 1 до 2000 символов".to_string(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO chat_messages (id, sender_id, recipient_id, body) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(user.id)
    .bind(other)
    .bind(text)
    .execute(&state.pool)
    .await?;
    let item = sqlx::query_as::<_, ChatMessageItem>(
        r#"
        SELECT m.id, m.sender_id, u.username AS sender_username,
               u.display_name AS sender_display_name, m.body, m.created_at
        FROM chat_messages m
        JOIN users u ON u.id = m.sender_id
        WHERE m.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(item))
}

pub async fn team_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<ChatMessageItem>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2)",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if !member && user.role != "admin" {
        return Err(ApiError::Forbidden("Ты не состоишь в команде".to_string()));
    }
    let items = sqlx::query_as::<_, ChatMessageItem>(
        r#"
        SELECT * FROM (
            SELECT m.id, m.sender_id, u.username AS sender_username,
                   u.display_name AS sender_display_name, m.body, m.created_at
            FROM chat_messages m
            JOIN users u ON u.id = m.sender_id
            WHERE m.team_id = $1
            ORDER BY m.created_at DESC
            LIMIT 100
        ) messages
        ORDER BY created_at
        "#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn send_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<ChatSendRequest>,
) -> Result<Json<ChatMessageItem>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2)",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if !member && user.role != "admin" {
        return Err(ApiError::Forbidden("Ты не состоишь в команде".to_string()));
    }
    let text = body.body.trim();
    if text.is_empty() || text.chars().count() > 2000 {
        return Err(ApiError::BadRequest(
            "Сообщение: от 1 до 2000 символов".to_string(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO chat_messages (id, sender_id, team_id, body) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(user.id)
        .bind(team_id)
        .bind(text)
        .execute(&state.pool)
        .await?;
    let item = sqlx::query_as::<_, ChatMessageItem>(
        r#"
        SELECT m.id, m.sender_id, u.username AS sender_username,
               u.display_name AS sender_display_name, m.body, m.created_at
        FROM chat_messages m
        JOIN users u ON u.id = m.sender_id
        WHERE m.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(item))
}
