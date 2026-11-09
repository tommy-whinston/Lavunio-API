use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{MarkNotificationsRequest, NotificationItem, NotificationsResponse};
use crate::AppState;

pub async fn create(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    kind: &str,
    title: &str,
    body: &str,
    link_type: Option<&str>,
    link_id: Option<&str>,
) {
    if let Err(error) = sqlx::query(
        r#"
        INSERT INTO notifications (id, user_id, kind, title, body, link_type, link_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(kind)
    .bind(title)
    .bind(body)
    .bind(link_type)
    .bind(link_id)
    .execute(pool)
    .await
    {
        tracing::warn!("failed to create notification: {error}");
    }
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NotificationsResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let items = sqlx::query_as::<_, NotificationItem>(
        r#"
        SELECT id, kind, title, body, link_type, link_id, read_at, created_at
        FROM notifications
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let unread = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(NotificationsResponse { items, unread }))
}

pub async fn mark_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MarkNotificationsRequest>,
) -> Result<Json<NotificationsResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    if body.ids.is_empty() {
        sqlx::query("UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL")
            .bind(user.id)
            .execute(&state.pool)
            .await?;
    } else {
        sqlx::query("UPDATE notifications SET read_at = now() WHERE user_id = $1 AND id = ANY($2)")
            .bind(user.id)
            .bind(&body.ids)
            .execute(&state.pool)
            .await?;
    }
    let items = sqlx::query_as::<_, NotificationItem>(
        r#"
        SELECT id, kind, title, body, link_type, link_id, read_at, created_at
        FROM notifications
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let unread = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(NotificationsResponse { items, unread }))
}
