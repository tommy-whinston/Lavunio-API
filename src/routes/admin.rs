use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    AdminCollectionItem, AdminModeItem, AdminReviewItem, CreateCollectionRequest,
    ModerationHistoryItem, ModerationItem, RejectRequest, ReleaseAppealItem, ReportItem,
    ResolveAppealRequest, SimpleStatus,
};
use crate::routes::notifications;
use crate::AppState;

#[derive(Debug, FromRow)]
struct ReleaseStatusRow {
    mode_id: Uuid,
    status: String,
}

#[derive(Debug, FromRow)]
struct AppealRow {
    release_id: Uuid,
    status: String,
    release_status: String,
    mode_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryQuery {
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReportsQuery {
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReviewQueueQuery {
    #[serde(default)]
    status: Option<String>,
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
               r.changelog, r.created_at,
               (SELECT a.manifest->'scan' FROM release_assets a
                WHERE a.release_id = r.id AND a.kind = 'archive'
                ORDER BY a.created_at LIMIT 1) AS scan
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

pub async fn collections(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminCollectionItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;

    let items = sqlx::query_as::<_, AdminCollectionItem>(
        r#"
        SELECT c.id, c.slug, c.title, c.description, c.status, c.sort_order, c.created_at,
               COALESCE(
                   array_agg(m.slug ORDER BY cm.sort_order) FILTER (WHERE m.slug IS NOT NULL),
                   '{}'
               ) AS mode_slugs
        FROM collections c
        LEFT JOIN collection_modes cm ON cm.collection_id = c.id
        LEFT JOIN modes m ON m.id = cm.mode_id
        GROUP BY c.id
        ORDER BY c.sort_order, c.created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}

pub async fn create_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCollectionRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;

    let slug = body.slug.trim().to_lowercase();
    if !super::validate_slug(&slug) {
        return Err(ApiError::BadRequest(
            "Slug: 3–64 символа, строчные латинские буквы, цифры и дефис".to_string(),
        ));
    }
    if matches!(slug.as_str(), "featured" | "popular" | "new" | "updated") {
        return Err(ApiError::BadRequest(
            "Этот slug зарезервирован для автоматических подборок".to_string(),
        ));
    }
    let title = body.title.trim();
    if title.chars().count() < 3 || title.chars().count() > 120 {
        return Err(ApiError::BadRequest(
            "Название подборки должно быть от 3 до 120 символов".to_string(),
        ));
    }
    let description = body.description.trim();
    if description.chars().count() > 1000 {
        return Err(ApiError::BadRequest(
            "Описание подборки слишком длинное".to_string(),
        ));
    }

    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM collections WHERE slug = $1)")
            .bind(&slug)
            .fetch_one(&state.pool)
            .await?;
    if exists {
        return Err(ApiError::Conflict(
            "Подборка с таким slug уже существует".to_string(),
        ));
    }

    let mut mode_ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for mode_slug in body
        .mode_slugs
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        if !seen.insert(mode_slug.to_string()) {
            continue;
        }
        let mode_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM modes WHERE slug = $1 AND status = 'published'",
        )
        .bind(mode_slug)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| {
            ApiError::BadRequest(format!("Опубликованный режим {mode_slug} не найден"))
        })?;
        mode_ids.push(mode_id);
    }

    let collection_id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO collections (id, slug, title, description, status, sort_order) VALUES ($1, $2, $3, $4, 'published', $5)",
    )
    .bind(collection_id)
    .bind(&slug)
    .bind(title)
    .bind(description)
    .bind(body.sort_order.unwrap_or(0))
    .execute(&mut *tx)
    .await?;

    for (index, mode_id) in mode_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO collection_modes (collection_id, mode_id, sort_order) VALUES ($1, $2, $3)",
        )
        .bind(collection_id)
        .bind(mode_id)
        .bind(index as i32)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(Json(SimpleStatus {
        ok: true,
        status: "created".to_string(),
    }))
}

pub async fn delete_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(collection_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    let deleted = sqlx::query("DELETE FROM collections WHERE id = $1")
        .bind(collection_id)
        .execute(&state.pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound("Подборка не найдена".to_string()));
    }
    Ok(Json(SimpleStatus {
        ok: true,
        status: "deleted".to_string(),
    }))
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

pub async fn reports(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReportsQuery>,
) -> Result<Json<Vec<ReportItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    let status = query.status.unwrap_or_else(|| "open".to_string());
    let status = status.trim().to_string();

    let items = sqlx::query_as::<_, ReportItem>(
        r#"
        SELECT r.id, r.target_type, r.target_id, r.reason, r.status,
               u.display_name AS reporter_name, u.username AS reporter_username,
               m.title AS target_title, m.slug AS target_slug,
               r.created_at
        FROM reports r
        LEFT JOIN users u ON u.id = r.user_id
        LEFT JOIN modes m ON m.id = r.target_id AND r.target_type = 'mode'
        WHERE ($1 = '' OR r.status = $1)
        ORDER BY r.created_at DESC
        LIMIT 100
        "#,
    )
    .bind(&status)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}

pub async fn reviews(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewQueueQuery>,
) -> Result<Json<Vec<AdminReviewItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    let status = query.status.unwrap_or_else(|| "pending".to_string());
    let status = status.trim().to_string();

    let items = sqlx::query_as::<_, AdminReviewItem>(
        r#"
        SELECT r.id, m.title AS mode_title, m.slug AS mode_slug,
               r.rating, r.text,
               u.display_name AS author_name, u.username AS author_username,
               r.created_at
        FROM reviews r
        JOIN modes m ON m.id = r.mode_id
        JOIN users u ON u.id = r.user_id
        WHERE ($1 = '' OR r.status = $1)
        ORDER BY r.created_at DESC
        LIMIT 100
        "#,
    )
    .bind(&status)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(items))
}

pub async fn approve_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(review_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let updated =
        sqlx::query("UPDATE reviews SET status = 'published' WHERE id = $1 AND status = 'pending'")
            .bind(review_id)
            .execute(&state.pool)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "Отзыв не найден или уже рассмотрен".to_string(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'review', $3, 'review_approve', '')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(review_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(SimpleStatus {
        ok: true,
        status: "published".to_string(),
    }))
}

pub async fn reject_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(review_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let updated =
        sqlx::query("UPDATE reviews SET status = 'rejected' WHERE id = $1 AND status = 'pending'")
            .bind(review_id)
            .execute(&state.pool)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "Отзыв не найден или уже рассмотрен".to_string(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'review', $3, 'review_reject', '')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(review_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(SimpleStatus {
        ok: true,
        status: "rejected".to_string(),
    }))
}

pub async fn close_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(report_id): Path<Uuid>,
    Json(body): Json<RejectRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let note = body.note.trim();

    let updated =
        sqlx::query("UPDATE reports SET status = 'closed' WHERE id = $1 AND status <> 'closed'")
            .bind(report_id)
            .execute(&state.pool)
            .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "Жалоба не найдена или уже закрыта".to_string(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'report', $3, 'report_close', $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(report_id)
    .bind(note)
    .execute(&state.pool)
    .await?;

    tracing::info!("moderator {} closed report {}", admin.username, report_id);

    Ok(Json(SimpleStatus {
        ok: true,
        status: "closed".to_string(),
    }))
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
    if let Ok((author_id, slug)) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT author_id, slug FROM modes WHERE id = $1",
    )
    .bind(release.mode_id)
    .fetch_one(&state.pool)
    .await
    {
        notifications::create(
            &state.pool,
            author_id,
            "release_approved",
            "Релиз одобрен",
            "Твой релиз опубликован в каталоге",
            Some("mode"),
            Some(&slug),
        )
        .await;
    }

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
    if let Ok((author_id, slug)) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT author_id, slug FROM modes WHERE id = $1",
    )
    .bind(release.mode_id)
    .fetch_one(&state.pool)
    .await
    {
        notifications::create(
            &state.pool,
            author_id,
            "release_rejected",
            "Релиз отклонён",
            note,
            Some("mode"),
            Some(&slug),
        )
        .await;
    }

    Ok(Json(SimpleStatus {
        ok: true,
        status: "rejected".to_string(),
    }))
}

pub async fn appeals(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ReleaseAppealItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    let items = sqlx::query_as::<_, ReleaseAppealItem>(
        r#"
        SELECT a.id, a.release_id, r.version,
               m.title AS mode_title, m.slug AS mode_slug,
               u.display_name AS author_name, u.username AS author_username,
               a.message, r.rejection_note, a.created_at
        FROM release_appeals a
        JOIN releases r ON r.id = a.release_id
        JOIN modes m ON m.id = r.mode_id
        JOIN users u ON u.id = a.author_id
        WHERE a.status = 'open'
        ORDER BY a.created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn resolve_appeal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(appeal_id): Path<Uuid>,
    Json(body): Json<ResolveAppealRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let action = body.action.trim();
    if action != "accept" && action != "reject" {
        return Err(ApiError::BadRequest(
            "action: accept или reject".to_string(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let appeal = sqlx::query_as::<_, AppealRow>(
        r#"
        SELECT a.release_id, a.status,
               r.status AS release_status, r.mode_id
        FROM release_appeals a
        JOIN releases r ON r.id = a.release_id
        WHERE a.id = $1
        FOR UPDATE OF a
        "#,
    )
    .bind(appeal_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Апелляция не найдена".to_string()))?;

    if appeal.status != "open" {
        return Err(ApiError::Conflict(
            "Апелляция уже рассмотрена".to_string(),
        ));
    }
    if appeal.release_status != "rejected" {
        return Err(ApiError::BadRequest(
            "Релиз уже не находится в статусе отклонённого".to_string(),
        ));
    }

    let note = body.note.trim();
    if action == "accept" {
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
        .bind(appeal.release_id)
        .bind(admin.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE modes SET status = 'published', updated_at = now() WHERE id = $1")
            .bind(appeal.mode_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            UPDATE release_appeals
            SET status = 'accepted', resolution_note = $2,
                resolved_at = now(), resolved_by = $3
            WHERE id = $1
            "#,
        )
        .bind(appeal_id)
        .bind(note)
        .bind(admin.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
            VALUES ($1, $2, 'release', $3, 'appeal_accept', $4)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(admin.id)
        .bind(appeal.release_id)
        .bind(note)
        .execute(&mut *tx)
        .await?;
    } else {
        let note = if note.is_empty() {
            "Апелляция отклонена"
        } else {
            note
        };
        sqlx::query(
            r#"
            UPDATE release_appeals
            SET status = 'rejected', resolution_note = $2,
                resolved_at = now(), resolved_by = $3
            WHERE id = $1
            "#,
        )
        .bind(appeal_id)
        .bind(note)
        .bind(admin.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
            VALUES ($1, $2, 'release', $3, 'appeal_reject', $4)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(admin.id)
        .bind(appeal.release_id)
        .bind(note)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let status = if action == "accept" {
        "published"
    } else {
        "rejected"
    };
    tracing::info!(
        "moderator {} resolved appeal {}: {}",
        admin.username,
        appeal_id,
        status
    );

    Ok(Json(SimpleStatus {
        ok: true,
        status: status.to_string(),
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
