use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{FriendItem, FriendRequestItem, FriendRequests, SimpleStatus};
use crate::routes::notifications;
use crate::AppState;

async fn user_by_username(pool: &sqlx::PgPool, username: &str) -> Result<(Uuid, String), ApiError> {
    sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, display_name FROM users WHERE lower(username) = lower($1)",
    )
    .bind(username.trim())
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Игрок не найден".to_string()))
}

pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "online".to_string(),
    }))
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<FriendItem>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let items = sqlx::query_as::<_, FriendItem>(
        r#"
        SELECT u.id AS user_id, u.username, u.display_name, u.avatar_url, u.skin_url,
               COALESCE(u.last_seen_at > now() - interval '2 minutes', false) AS online,
               u.last_seen_at
        FROM friend_requests f
        JOIN users u ON u.id = CASE
            WHEN f.requester_id = $1 THEN f.addressee_id
            ELSE f.requester_id
        END
        WHERE f.status = 'accepted'
          AND (f.requester_id = $1 OR f.addressee_id = $1)
        ORDER BY online DESC, u.display_name
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FriendRequests>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let incoming = sqlx::query_as::<_, FriendRequestItem>(
        r#"
        SELECT f.id, u.id AS user_id, u.username, u.display_name, u.avatar_url, f.created_at
        FROM friend_requests f
        JOIN users u ON u.id = f.requester_id
        WHERE f.addressee_id = $1 AND f.status = 'pending'
        ORDER BY f.created_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let outgoing = sqlx::query_as::<_, FriendRequestItem>(
        r#"
        SELECT f.id, u.id AS user_id, u.username, u.display_name, u.avatar_url, f.created_at
        FROM friend_requests f
        JOIN users u ON u.id = f.addressee_id
        WHERE f.requester_id = $1 AND f.status = 'pending'
        ORDER BY f.created_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(FriendRequests { incoming, outgoing }))
}

pub async fn add(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let (friend_id, friend_name) = user_by_username(&state.pool, &username).await?;
    if friend_id == user.id {
        return Err(ApiError::BadRequest(
            "Нельзя добавить себя в друзья".to_string(),
        ));
    }

    let existing = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        r#"
        SELECT id, requester_id, status FROM friend_requests
        WHERE (requester_id = $1 AND addressee_id = $2)
           OR (requester_id = $2 AND addressee_id = $1)
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user.id)
    .bind(friend_id)
    .fetch_optional(&state.pool)
    .await?;

    if let Some((request_id, requester_id, status)) = existing {
        if status == "accepted" {
            return Err(ApiError::Conflict("Вы уже друзья".to_string()));
        }
        if status == "pending" {
            if requester_id == user.id {
                return Err(ApiError::Conflict("Заявка уже отправлена".to_string()));
            }
            // Встречная заявка — сразу принимаем.
            sqlx::query(
                "UPDATE friend_requests SET status = 'accepted', responded_at = now() WHERE id = $1",
            )
            .bind(request_id)
            .execute(&state.pool)
            .await?;
            notifications::create(
                &state.pool,
                friend_id,
                "friend_accept",
                "Заявка в друзья принята",
                &format!("{} теперь у тебя в друзьях", user.display_name),
                Some("user"),
                Some(&user.username),
            )
            .await;
            return Ok(Json(SimpleStatus {
                ok: true,
                status: "accepted".to_string(),
            }));
        }
    }

    let request_id = Uuid::new_v4();
    sqlx::query("INSERT INTO friend_requests (id, requester_id, addressee_id) VALUES ($1, $2, $3)")
        .bind(request_id)
        .bind(user.id)
        .bind(friend_id)
        .execute(&state.pool)
        .await?;
    notifications::create(
        &state.pool,
        friend_id,
        "friend_request",
        "Заявка в друзья",
        &format!("{} хочет добавить тебя в друзья", user.display_name),
        Some("user"),
        Some(&user.username),
    )
    .await;
    let _ = friend_name;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "pending".to_string(),
    }))
}

pub async fn respond(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((request_id, action)): Path<(Uuid, String)>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let accept = action == "accept";
    let row = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "SELECT requester_id, addressee_id, status FROM friend_requests WHERE id = $1",
    )
    .bind(request_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Заявка не найдена".to_string()))?;
    if row.1 != user.id {
        return Err(ApiError::Forbidden(
            "Эта заявка адресована другому пользователю".to_string(),
        ));
    }
    if row.2 != "pending" {
        return Err(ApiError::Conflict("Заявка уже обработана".to_string()));
    }
    sqlx::query("UPDATE friend_requests SET status = $2, responded_at = now() WHERE id = $1")
        .bind(request_id)
        .bind(if accept { "accepted" } else { "declined" })
        .execute(&state.pool)
        .await?;
    if accept {
        notifications::create(
            &state.pool,
            row.0,
            "friend_accept",
            "Заявка в друзья принята",
            &format!("{} принял заявку в друзья", user.display_name),
            Some("user"),
            Some(&user.username),
        )
        .await;
    }
    Ok(Json(SimpleStatus {
        ok: true,
        status: if accept { "accepted" } else { "declined" }.to_string(),
    }))
}

pub async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let (friend_id, _) = user_by_username(&state.pool, &username).await?;
    sqlx::query(
        r#"
        DELETE FROM friend_requests
        WHERE status = 'accepted'
          AND ((requester_id = $1 AND addressee_id = $2)
            OR (requester_id = $2 AND addressee_id = $1))
        "#,
    )
    .bind(user.id)
    .bind(friend_id)
    .execute(&state.pool)
    .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "removed".to_string(),
    }))
}
