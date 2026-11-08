use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{
    Asset, AuthorResponse, BuildInfo, CatalogResponse, CollectionSection, ModeRow, ModeSummary,
    Release, ReleaseSummary, Tag, UserSummary,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogQuery {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub author: Option<String>,
    pub sort: Option<String>,
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

#[derive(Debug, FromRow)]
struct BuildRow {
    size: i64,
    manifest: Option<Value>,
    version: String,
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

async fn latest_build(pool: &PgPool, mode_id: Uuid) -> Result<Option<BuildInfo>, ApiError> {
    let row = sqlx::query_as::<_, BuildRow>(
        r#"
        SELECT a.size, a.manifest, r.version
        FROM release_assets a
        JOIN releases r ON r.id = a.release_id
        WHERE r.mode_id = $1 AND r.status = 'published' AND a.kind = 'archive'
        ORDER BY r.published_at DESC NULLS LAST, a.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(mode_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| {
        let files = row
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.get("files"))
            .and_then(Value::as_object)
            .map(|files| files.len() as u64)
            .unwrap_or(0);
        let bytes = row
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.get("archive"))
            .and_then(|archive| archive.get("bytes"))
            .and_then(Value::as_u64)
            .unwrap_or(row.size.max(0) as u64);
        BuildInfo {
            files,
            bytes,
            version: Some(row.version),
        }
    }))
}

async fn mode_summary(pool: &PgPool, row: ModeRow) -> Result<ModeSummary, ApiError> {
    let tags = tags_for(pool, row.id).await?;
    let latest_release = latest_release(pool, row.id).await?;
    let build = latest_build(pool, row.id).await?;
    let installs = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM mode_events WHERE mode_id = $1 AND event = 'install'",
    )
    .bind(row.id)
    .fetch_one(pool)
    .await?;
    let screenshots = sqlx::query_scalar::<_, String>(
        r#"
        SELECT a.url
        FROM release_assets a
        JOIN releases r ON r.id = a.release_id
        WHERE r.mode_id = $1
          AND r.status = 'published'
          AND a.kind = 'screenshot'
          AND a.url <> ''
        ORDER BY r.published_at DESC NULLS LAST, a.created_at
        LIMIT 12
        "#,
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;
    let (rating, reviews_count) = sqlx::query_as::<_, (Option<f64>, i64)>(
        r#"
        SELECT AVG(rating)::float8, COUNT(*)
        FROM reviews
        WHERE mode_id = $1 AND status = 'published'
        "#,
    )
    .bind(row.id)
    .fetch_one(pool)
    .await?;
    Ok(ModeSummary {
        id: row.id,
        slug: row.slug,
        title: row.title,
        short_description: row.short_description,
        description: row.description,
        status: row.status,
        cover_url: row.cover_url,
        trailer_url: row.trailer_url,
        mc_version: row.mc_version,
        loader: row.loader,
        loader_version: row.loader_version,
        size: row.size_label,
        installs,
        rating,
        reviews_count,
        screenshots,
        author: UserSummary {
            id: row.author_id,
            username: row.author_username,
            display_name: row.author_name,
            avatar_url: row.author_avatar,
        },
        tags,
        latest_release,
        build,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn mode_row_by_slug(pool: &PgPool, slug: &str) -> Result<ModeRow, ApiError> {
    sqlx::query_as::<_, ModeRow>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.description, m.status,
               m.cover_url, m.trailer_url, m.mc_version, m.loader, m.loader_version, m.size_label,
               m.created_at, m.updated_at,
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
        SELECT id, kind, url, size, sha256, parts, manifest,
               file_name, storage_key, storage_kind
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
    let mc_version = query.mc_version.unwrap_or_default().trim().to_string();
    let loader = query.loader.unwrap_or_default().trim().to_lowercase();
    let author = query.author.unwrap_or_default().trim().to_string();
    let order = match query.sort.as_deref().unwrap_or("updated") {
        "title" => "m.title ASC",
        "newest" => "m.created_at DESC",
        "installs" => {
            "(SELECT COUNT(*) FROM mode_events e WHERE e.mode_id = m.id AND e.event = 'install') DESC, m.updated_at DESC"
        }
        _ => "m.updated_at DESC",
    };

    let sql = format!(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.description, m.status,
               m.cover_url, m.trailer_url, m.mc_version, m.loader, m.loader_version, m.size_label,
               m.created_at, m.updated_at,
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
          AND ($3 = '' OR m.mc_version = $3)
          AND ($4 = '' OR m.loader = $4)
          AND (
              $5 = ''
              OR u.username ILIKE '%' || $5 || '%'
              OR u.display_name ILIKE '%' || $5 || '%'
          )
        ORDER BY {order}
        LIMIT $6 OFFSET $7
        "#
    );

    let rows = sqlx::query_as::<_, ModeRow>(&sql)
        .bind(&search)
        .bind(&tag)
        .bind(&mc_version)
        .bind(&loader)
        .bind(&author)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
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
          AND ($3 = '' OR m.mc_version = $3)
          AND ($4 = '' OR m.loader = $4)
          AND (
              $5 = ''
              OR u.username ILIKE '%' || $5 || '%'
              OR u.display_name ILIKE '%' || $5 || '%'
          )
        "#,
    )
    .bind(&search)
    .bind(&tag)
    .bind(&mc_version)
    .bind(&loader)
    .bind(&author)
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

fn mode_rating(mode: &ModeSummary) -> f64 {
    mode.rating.unwrap_or(0.0)
}

pub async fn collections(
    State(state): State<AppState>,
) -> Result<Json<Vec<CollectionSection>>, ApiError> {
    let rows = sqlx::query_as::<_, ModeRow>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.description, m.status,
               m.cover_url, m.trailer_url, m.mc_version, m.loader, m.loader_version, m.size_label,
               m.created_at, m.updated_at,
               u.id AS author_id, u.username AS author_username,
               u.display_name AS author_name, u.avatar_url AS author_avatar
        FROM modes m
        JOIN users u ON u.id = m.author_id
        WHERE m.status = 'published'
        ORDER BY m.updated_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut modes = Vec::with_capacity(rows.len());
    for row in rows {
        modes.push(mode_summary(&state.pool, row).await?);
    }

    if modes.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let mut popular = modes.clone();
    popular.sort_by(|a, b| {
        b.installs
            .cmp(&a.installs)
            .then_with(|| mode_rating(b).total_cmp(&mode_rating(a)))
            .then_with(|| a.title.cmp(&b.title))
    });
    popular.truncate(6);

    let mut newest = modes.clone();
    newest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    newest.truncate(6);

    let mut updated = modes.clone();
    updated.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    updated.truncate(6);

    let mut featured: Vec<ModeSummary> = modes
        .iter()
        .filter(|mode| mode.reviews_count > 0)
        .cloned()
        .collect();
    featured.sort_by(|a, b| {
        mode_rating(b)
            .total_cmp(&mode_rating(a))
            .then_with(|| b.installs.cmp(&a.installs))
    });
    if featured.is_empty() {
        featured = popular.clone();
    }
    featured.truncate(6);

    Ok(Json(vec![
        CollectionSection {
            id: "featured".to_string(),
            title: "Рекомендуем".to_string(),
            modes: featured,
        },
        CollectionSection {
            id: "popular".to_string(),
            title: "Популярное".to_string(),
            modes: popular,
        },
        CollectionSection {
            id: "new".to_string(),
            title: "Новинки".to_string(),
            modes: newest,
        },
        CollectionSection {
            id: "updated".to_string(),
            title: "Обновления".to_string(),
            modes: updated,
        },
    ]))
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
        SELECT id, version, status, channel, changelog, published_at, created_at,
               rejection_note, moderated_at
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
               m.cover_url, m.trailer_url, m.mc_version, m.loader, m.loader_version, m.size_label,
               m.created_at, m.updated_at,
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
