use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{ReportRequest, ReviewItem, ReviewListResponse, ReviewRequest, SimpleStatus};
use crate::AppState;

#[derive(Debug, FromRow)]
struct ReviewStatsRow {
    average_rating: Option<f64>,
    count: i64,
}

async fn published_mode_id(pool: &PgPool, slug: &str) -> Result<Uuid, ApiError> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM modes WHERE slug = $1 AND status = 'published'")
        .bind(slug.trim())
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))
}

async fn review_list(pool: &PgPool, mode_id: Uuid) -> Result<ReviewListResponse, ApiError> {
    let stats = sqlx::query_as::<_, ReviewStatsRow>(
        r#"
        SELECT AVG(rating)::float8 AS average_rating, COUNT(*) AS count
        FROM reviews
        WHERE mode_id = $1 AND status = 'published'
        "#,
    )
    .bind(mode_id)
    .fetch_one(pool)
    .await?;

    let items = sqlx::query_as::<_, ReviewItem>(
        r#"
        SELECT r.id, r.rating, r.text,
               u.display_name AS author_name, u.username AS author_username,
               u.avatar_url, r.created_at
        FROM reviews r
        JOIN users u ON u.id = r.user_id
        WHERE r.mode_id = $1 AND r.status = 'published'
        ORDER BY r.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(mode_id)
    .fetch_all(pool)
    .await?;

    Ok(ReviewListResponse {
        average_rating: stats.average_rating,
        count: stats.count,
        items,
    })
}

pub async fn wishlist(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let items = sqlx::query_scalar::<_, String>(
        r#"
        SELECT m.slug
        FROM wishlist w
        JOIN modes m ON m.id = w.mode_id
        WHERE w.user_id = $1 AND m.status = 'published'
        ORDER BY w.created_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn add_wishlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let mode_id = published_mode_id(&state.pool, &slug).await?;
    sqlx::query("INSERT INTO wishlist (user_id, mode_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user.id)
        .bind(mode_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "wishlisted".to_string(),
    }))
}

pub async fn remove_wishlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let mode_id = published_mode_id(&state.pool, &slug).await?;
    sqlx::query("DELETE FROM wishlist WHERE user_id = $1 AND mode_id = $2")
        .bind(user.id)
        .bind(mode_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "removed".to_string(),
    }))
}

pub async fn reviews(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<ReviewListResponse>, ApiError> {
    let mode_id = published_mode_id(&state.pool, &slug).await?;
    Ok(Json(review_list(&state.pool, mode_id).await?))
}

pub async fn submit_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<ReviewRequest>,
) -> Result<Json<ReviewListResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    if !(1..=5).contains(&body.rating) {
        return Err(ApiError::BadRequest(
            "Оценка должна быть от 1 до 5".to_string(),
        ));
    }
    let text = body.text.trim();
    if text.chars().count() > 2000 {
        return Err(ApiError::BadRequest(
            "Отзыв должен быть не длиннее 2000 символов".to_string(),
        ));
    }
    let mode_id = published_mode_id(&state.pool, &slug).await?;

    sqlx::query(
        r#"
        INSERT INTO reviews (id, mode_id, user_id, rating, text, status, created_at)
        VALUES ($1, $2, $3, $4, $5, 'published', now())
        ON CONFLICT (mode_id, user_id) DO UPDATE
            SET rating = EXCLUDED.rating,
                text = EXCLUDED.text,
                status = 'published',
                created_at = now()
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(mode_id)
    .bind(user.id)
    .bind(body.rating)
    .bind(text)
    .execute(&state.pool)
    .await?;

    Ok(Json(review_list(&state.pool, mode_id).await?))
}

pub async fn delete_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let mode_id = published_mode_id(&state.pool, &slug).await?;
    sqlx::query("DELETE FROM reviews WHERE mode_id = $1 AND user_id = $2")
        .bind(mode_id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "deleted".to_string(),
    }))
}

pub async fn submit_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<ReportRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let reason = body.reason.trim();
    if reason.chars().count() < 3 {
        return Err(ApiError::BadRequest(
            "Опиши причину жалобы (минимум 3 символа)".to_string(),
        ));
    }
    if reason.chars().count() > 1000 {
        return Err(ApiError::BadRequest(
            "Причина жалобы слишком длинная".to_string(),
        ));
    }
    let mode_id = published_mode_id(&state.pool, &slug).await?;
    sqlx::query(
        "INSERT INTO reports (id, target_type, target_id, user_id, reason, status) VALUES ($1, 'mode', $2, $3, $4, 'open')",
    )
    .bind(Uuid::new_v4())
    .bind(mode_id)
    .bind(user.id)
    .bind(reason)
    .execute(&state.pool)
    .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "reported".to_string(),
    }))
}
