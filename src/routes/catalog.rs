use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{
    Asset, AuthorResponse, CatalogResponse, ModeRow, ModeSummary, Release, ReleaseSummary, Tag,
    UserSummary,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CatalogQuery {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, FromRow)]
struct AuthorRow {
    id: Uuid,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
}

async fn tags_for(pool: &PgPool, mode_id: Uuid) -> Result<Vec<Tag>, ApiError> {
    let tags = sqlx::query_as::<_, Tag>(
        r#"
        SELECT t.id, t.slug, t.name
        FROM tags t
        JOIN mode_tags mt ON mt.tag_id = t.id
        WHERE mt.mode_id = $1
        ORDER BY t.name
        "#,
    )
    .bind(mode_id)
    .fetch_all(pool)
    .await?;
    Ok(tags)
}

async fn latest_release(pool: &PgPool, mode_id: Uuid) -> Result<Option<ReleaseSummary>, ApiError> {
    let release = sqlx::query_as::<_, ReleaseSummary>(
        r#"
        SELECT id, version, status, channel, published_at
        FROM releases
        WHERE mode_id = $1 AND status = 'published'
        ORDER BY published_at DESC NULLS LAST, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(mode_id)
    .fetch_optional(pool)
    .await?;
    Ok(release)
}

async fn mode_summary(pool: &PgPool, row: ModeRow) -> Result<ModeSummary, ApiError> {
    let tags = tags_for(pool, row.id).await?;
    let latest_release = latest_release(pool, row.id).await?;
    Ok(ModeSummary {
        id: row.id,
        slug: row.slug,
        title: row.title,
        short_description: row.short_description,
        description: row.description,
        status: row.status,
        cover_url: row.cover_url,
        author: UserSummary {
            id: row.author_id,
            username: row.author_username,
            display_name: row.author_name,
            avatar_url: row.author_avatar,
        },
        tags,
        latest_release,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn mode_row_by_slug(pool: &PgPool, slug: &str) -> Result<ModeRow, ApiError> {
    sqlx::query_as::<_, ModeRow>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.description, m.status,
               m.cover_url, m.created_at, m.updated_at,
               u.id AS author_id, u.username AS author_username,
               u.display_name AS author_name, u.avatar_url AS author_avatar
        FROM modes m
        JOIN users u ON u.id = m.author_id
        WHERE m.slug = $1 AND m.status = 'published'
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))
}

async fn assets_for(pool: &PgPool, release_id: Uuid) -> Result<Vec<Asset>, ApiError> {
    let assets = sqlx::query_as::<_, Asset>(
        r#"
        SELECT id, kind, url, size, sha256, parts, manifest
        FROM release_assets
        WHERE release_id = $1
        ORDER BY created_at
        "#,
    )
    .bind(release_id)
    .fetch_all(pool)
    .await?;
    Ok(assets)
}

pub async fn catalog(
    State(state): State<AppState>,
    Query(query): Query<CatalogQuery>,
) -> Result<Json<CatalogResponse>, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;
    let search = query.q.unwrap_or_default().trim().to_string();
    let tag = query.tag.unwrap_or_default().trim().to_string();

    let rows = sqlx::query_as::<_, ModeRow>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.description, m.status,
               m.cover_url, m.created_at, m.updated_at,
               u.id AS author_id, u.username AS author_username,
               u.display_name AS author_name, u.avatar_url AS author_avatar
        FROM modes m
        JOIN users u ON u.id = m.author_id
        WHERE m.status = 'published'
          AND (
              $1 = ''
              OR m.title ILIKE '%' || $1 || '%'
              OR m.short_description ILIKE '%' || $1 || '%'
              OR m.description ILIKE '%' || $1 || '%'
          )
          AND (
              $2 = ''
              OR EXISTS (
                  SELECT 1
                  FROM mode_tags mt
                  JOIN tags t ON t.id = mt.tag_id
                  WHERE mt.mode_id = m.id AND t.slug = $2
              )
          )
        ORDER BY m.updated_at DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(&search)
    .bind(&tag)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM modes m
        WHERE m.status = 'published'
          AND (
              $1 = ''
              OR m.title ILIKE '%' || $1 || '%'
              OR m.short_description ILIKE '%' || $1 || '%'
              OR m.description ILIKE '%' || $1 || '%'
          )
          AND (
              $2 = ''
              OR EXISTS (
                  SELECT 1
                  FROM mode_tags mt
                  JOIN tags t ON t.id = mt.tag_id
                  WHERE mt.mode_id = m.id AND t.slug = $2
              )
          )
        "#,
    )
    .bind(&search)
    .bind(&tag)
    .fetch_one(&state.pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(mode_summary(&state.pool, row).await?);
    }

    Ok(Json(CatalogResponse {
        items,
        page,
        limit,
        total,
    }))
}

pub async fn mode_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<ModeSummary>, ApiError> {
    let row = mode_row_by_slug(&state.pool, &slug).await?;
    Ok(Json(mode_summary(&state.pool, row).await?))
}

pub async fn releases(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Vec<Release>>, ApiError> {
    let mode = mode_row_by_slug(&state.pool, &slug).await?;
    let mut releases = sqlx::query_as::<_, Release>(
        r#"
        SELECT id, version, status, channel, changelog, published_at, created_at
        FROM releases
        WHERE mode_id = $1 AND status = 'published'
        ORDER BY published_at DESC NULLS LAST, created_at DESC
        "#,
    )
    .bind(mode.id)
    .fetch_all(&state.pool)
    .await?;

    for release in &mut releases {
        release.assets = assets_for(&state.pool, release.id).await?;
    }

    Ok(Json(releases))
}

pub async fn author(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<AuthorResponse>, ApiError> {
    let user = sqlx::query_as::<_, AuthorRow>(
        "SELECT id, username, display_name, avatar_url FROM users WHERE username = $1",
    )
    .bind(&slug)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Автор не найден".to_string()))?;

    let rows = sqlx::query_as::<_, ModeRow>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.description, m.status,
               m.cover_url, m.created_at, m.updated_at,
               u.id AS author_id, u.username AS author_username,
               u.display_name AS author_name, u.avatar_url AS author_avatar
        FROM modes m
        JOIN users u ON u.id = m.author_id
        WHERE m.author_id = $1 AND m.status = 'published'
        ORDER BY m.updated_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let mut modes = Vec::with_capacity(rows.len());
    for row in rows {
        modes.push(mode_summary(&state.pool, row).await?);
    }

    Ok(Json(AuthorResponse {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        modes,
    }))
}
