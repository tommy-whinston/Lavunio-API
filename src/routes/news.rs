use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{NewsItem, NewsRequest, SimpleStatus};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct NewsQuery {
    #[serde(default)]
    limit: Option<i64>,
}

fn validate_news(body: &NewsRequest) -> Result<(String, String), ApiError> {
    let title = body.title.trim();
    let text = body.body.trim();
    if title.chars().count() < 3 {
        return Err(ApiError::BadRequest(
            "Заголовок новости слишком короткий".to_string(),
        ));
    }
    if title.chars().count() > 120 {
        return Err(ApiError::BadRequest(
            "Заголовок новости слишком длинный".to_string(),
        ));
    }
    if text.chars().count() < 3 {
        return Err(ApiError::BadRequest(
            "Текст новости слишком короткий".to_string(),
        ));
    }
    if text.chars().count() > 5000 {
        return Err(ApiError::BadRequest(
            "Текст новости слишком длинный".to_string(),
        ));
    }
    Ok((title.to_string(), text.to_string()))
}

async fn news_list(
    pool: &PgPool,
    mode_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<NewsItem>, ApiError> {
    let items = sqlx::query_as::<_, NewsItem>(
        r#"
        SELECT n.id, n.mode_id, m.slug AS mode_slug, m.title AS mode_title,
               n.title, n.body,
               u.display_name AS author_name, u.username AS author_username,
               n.created_at
        FROM news n
        JOIN users u ON u.id = n.author_id
        LEFT JOIN modes m ON m.id = n.mode_id
        WHERE n.status = 'published'
          AND (($1::uuid IS NULL AND n.mode_id IS NULL) OR n.mode_id = $1)
        ORDER BY n.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(mode_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

pub async fn store_news(
    State(state): State<AppState>,
    Query(query): Query<NewsQuery>,
) -> Result<Json<Vec<NewsItem>>, ApiError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    Ok(Json(news_list(&state.pool, None, limit).await?))
}

pub async fn mode_news(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<NewsQuery>,
) -> Result<Json<Vec<NewsItem>>, ApiError> {
    let mode_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM modes WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))?;
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    Ok(Json(news_list(&state.pool, Some(mode_id), limit).await?))
}

pub async fn create_store_news(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<NewsRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_admin(&state.pool, &headers).await?;
    let (title, text) = validate_news(&body)?;
    sqlx::query(
        "INSERT INTO news (id, mode_id, author_id, title, body, status) VALUES ($1, NULL, $2, $3, $4, 'published')",
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(title)
    .bind(text)
    .execute(&state.pool)
    .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "published".to_string(),
    }))
}

pub async fn create_mode_news(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
    Json(body): Json<NewsRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let author_id = sqlx::query_scalar::<_, Uuid>("SELECT author_id FROM modes WHERE id = $1")
        .bind(mode_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))?;
    if author_id != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden(
            "Новости может публиковать только автор режима".to_string(),
        ));
    }
    let (title, text) = validate_news(&body)?;
    sqlx::query(
        "INSERT INTO news (id, mode_id, author_id, title, body, status) VALUES ($1, $2, $3, $4, $5, 'published')",
    )
    .bind(Uuid::new_v4())
    .bind(mode_id)
    .bind(user.id)
    .bind(title)
    .bind(text)
    .execute(&state.pool)
    .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "published".to_string(),
    }))
}

pub async fn delete_news(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(news_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let author_id = sqlx::query_scalar::<_, Uuid>("SELECT author_id FROM news WHERE id = $1")
        .bind(news_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Новость не найдена".to_string()))?;
    if author_id != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden(
            "Удалить новость может только автор или администратор".to_string(),
        ));
    }
    sqlx::query("DELETE FROM news WHERE id = $1")
        .bind(news_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "deleted".to_string(),
    }))
}
