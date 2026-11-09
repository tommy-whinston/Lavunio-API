use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{CommentItem, CommentRequest, SimpleStatus};
use crate::routes::notifications;
use crate::AppState;

async fn mode_id_by_slug(pool: &sqlx::PgPool, slug: &str) -> Result<Uuid, ApiError> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM modes WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))
}

pub async fn list(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Vec<CommentItem>>, ApiError> {
    let items = sqlx::query_as::<_, CommentItem>(
        r#"
        SELECT c.id, c.parent_id, c.body, c.user_id,
               u.username, u.display_name, u.avatar_url, c.created_at
        FROM mode_comments c
        JOIN users u ON u.id = c.user_id
        JOIN modes m ON m.id = c.mode_id
        WHERE m.slug = $1 AND c.status = 'published'
        ORDER BY c.created_at
        LIMIT 200
        "#,
    )
    .bind(slug)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<CommentRequest>,
) -> Result<Json<CommentItem>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let mode_id = mode_id_by_slug(&state.pool, &slug).await?;
    let text = body.body.trim();
    if text.is_empty() || text.chars().count() > 2000 {
        return Err(ApiError::BadRequest(
            "Комментарий: от 1 до 2000 символов".to_string(),
        ));
    }
    if let Some(parent_id) = body.parent_id {
        let parent = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM mode_comments WHERE id = $1 AND mode_id = $2 AND status = 'published')",
        )
        .bind(parent_id)
        .bind(mode_id)
        .fetch_one(&state.pool)
        .await?;
        if !parent {
            return Err(ApiError::BadRequest("Родительский комментарий не найден".to_string()));
        }
    }

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mode_comments (id, mode_id, user_id, parent_id, body) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(mode_id)
    .bind(user.id)
    .bind(body.parent_id)
    .bind(text)
    .execute(&state.pool)
    .await?;

    if let Some(author_id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT author_id FROM modes WHERE id = $1",
    )
    .bind(mode_id)
    .fetch_optional(&state.pool)
    .await?
    {
        if author_id != user.id {
            notifications::create(
                &state.pool,
                author_id,
                "comment",
                "Новый комментарий",
                &format!("{} оставил комментарий к режиму", user.display_name),
                Some("mode"),
                Some(&slug),
            )
            .await;
        }
    }

    let item = sqlx::query_as::<_, CommentItem>(
        r#"
        SELECT c.id, c.parent_id, c.body, c.user_id,
               u.username, u.display_name, u.avatar_url, c.created_at
        FROM mode_comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(item))
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(comment_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let owner = sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM mode_comments WHERE id = $1")
        .bind(comment_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Комментарий не найден".to_string()))?;
    if owner != user.id && user.role != "admin" && user.role != "moderator" {
        return Err(ApiError::Forbidden(
            "Можно удалять только свои комментарии".to_string(),
        ));
    }
    sqlx::query("UPDATE mode_comments SET status = 'hidden', updated_at = now() WHERE id = $1")
        .bind(comment_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "deleted".to_string(),
    }))
}
