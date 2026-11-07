use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{ModerationItem, RejectRequest, SimpleStatus};
use crate::AppState;

#[derive(Debug, FromRow)]
struct ReleaseStatusRow {
    mode_id: Uuid,
    status: String,
}

pub async fn queue(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ModerationItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;

    let items = sqlx::query_as::<_, ModerationItem>(
        r#"
        SELECT r.id AS release_id, r.version,
               m.id AS mode_id, m.slug AS mode_slug, m.title AS mode_title,
               u.display_name AS author_name, r.created_at
        FROM releases r
        JOIN modes m ON m.id = r.mode_id
        JOIN users u ON u.id = m.author_id
        WHERE r.status = 'pending'
        ORDER BY r.created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}

async fn release_status(
    pool: &sqlx::PgPool,
    release_id: Uuid,
) -> Result<ReleaseStatusRow, ApiError> {
    sqlx::query_as::<_, ReleaseStatusRow>("SELECT mode_id, status FROM releases WHERE id = $1")
        .bind(release_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Релиз не найден".to_string()))
}

pub async fn approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(release_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let release = release_status(&state.pool, release_id).await?;
    if release.status != "pending" {
        return Err(ApiError::BadRequest(
            "Одобрить можно только релиз на модерации".to_string(),
        ));
    }

    sqlx::query("UPDATE releases SET status = 'published', published_at = now() WHERE id = $1")
        .bind(release_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE modes SET status = 'published', updated_at = now() WHERE id = $1")
        .bind(release.mode_id)
        .execute(&state.pool)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'release', $3, 'approve', '')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(release_id)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "moderator {} approved release {}",
        admin.username,
        release_id
    );

    Ok(Json(SimpleStatus {
        ok: true,
        status: "published".to_string(),
    }))
}

pub async fn reject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(release_id): Path<Uuid>,
    Json(body): Json<RejectRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let release = release_status(&state.pool, release_id).await?;
    if release.status != "pending" {
        return Err(ApiError::BadRequest(
            "Отклонить можно только релиз на модерации".to_string(),
        ));
    }

    sqlx::query("UPDATE releases SET status = 'rejected' WHERE id = $1")
        .bind(release_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE modes SET status = 'draft', updated_at = now() WHERE id = $1")
        .bind(release.mode_id)
        .execute(&state.pool)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'release', $3, 'reject', $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(release_id)
    .bind(body.note.trim())
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "moderator {} rejected release {}",
        admin.username,
        release_id
    );

    Ok(Json(SimpleStatus {
        ok: true,
        status: "rejected".to_string(),
    }))
}
