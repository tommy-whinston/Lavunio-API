use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    AdminModeItem, ModerationHistoryItem, ModerationItem, RejectRequest, SimpleStatus,
};
use crate::AppState;

#[derive(Debug, FromRow)]
struct ReleaseStatusRow {
    mode_id: Uuid,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    limit: Option<i64>,
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
               u.display_name AS author_name, u.username AS author_username,
               r.changelog, r.created_at
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

pub async fn modes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminModeItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;

    let items = sqlx::query_as::<_, AdminModeItem>(
        r#"
        SELECT m.id, m.slug, m.title, m.status,
               u.display_name AS author_name, u.username AS author_username,
               m.created_at, m.updated_at
        FROM modes m
        JOIN users u ON u.id = m.author_id
        ORDER BY m.updated_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}

pub async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<ModerationHistoryItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let items = sqlx::query_as::<_, ModerationHistoryItem>(
        r#"
        SELECT a.id, a.action, a.note, a.target_type, a.target_id,
               u.display_name AS moderator_name, a.created_at
        FROM moderation_actions a
        JOIN users u ON u.id = a.moderator_id
        ORDER BY a.created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
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

    sqlx::query(
        r#"
        UPDATE releases
        SET status = 'published',
            published_at = now(),
            rejection_note = NULL,
            moderated_at = now(),
            moderated_by = $2
        WHERE id = $1
        "#,
    )
    .bind(release_id)
    .bind(admin.id)
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
    let note = body.note.trim();
    if note.is_empty() {
        return Err(ApiError::BadRequest("Укажи причину отклонения".to_string()));
    }
    let release = release_status(&state.pool, release_id).await?;
    if release.status != "pending" {
        return Err(ApiError::BadRequest(
            "Отклонить можно только релиз на модерации".to_string(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE releases
        SET status = 'rejected',
            rejection_note = $2,
            moderated_at = now(),
            moderated_by = $3
        WHERE id = $1
        "#,
    )
    .bind(release_id)
    .bind(note)
    .bind(admin.id)
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
    .bind(note)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "moderator {} rejected release {}: {}",
        admin.username,
        release_id,
        note
    );

    Ok(Json(SimpleStatus {
        ok: true,
        status: "rejected".to_string(),
    }))
}

pub async fn hide_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
    Json(body): Json<RejectRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let note = body.note.trim();
    if note.is_empty() {
        return Err(ApiError::BadRequest("Укажи причину скрытия".to_string()));
    }

    let updated =
        sqlx::query("UPDATE modes SET status = 'hidden', updated_at = now() WHERE id = $1")
            .bind(mode_id)
            .execute(&state.pool)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound("Режим не найден".to_string()));
    }

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'mode', $3, 'hide', $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(mode_id)
    .bind(note)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "moderator {} hid mode {}: {}",
        admin.username,
        mode_id,
        note
    );

    Ok(Json(SimpleStatus {
        ok: true,
        status: "hidden".to_string(),
    }))
}

pub async fn unhide_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;

    let updated =
        sqlx::query("UPDATE modes SET status = 'published', updated_at = now() WHERE id = $1")
            .bind(mode_id)
            .execute(&state.pool)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound("Режим не найден".to_string()));
    }

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'mode', $3, 'unhide', '')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(mode_id)
    .execute(&state.pool)
    .await?;

    tracing::info!("moderator {} unhid mode {}", admin.username, mode_id);

    Ok(Json(SimpleStatus {
        ok: true,
        status: "published".to_string(),
    }))
}
