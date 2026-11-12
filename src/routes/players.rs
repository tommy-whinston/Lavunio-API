use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    CreateProfilePostRequest, PlayerModeItem, PlayerProfile, PlayerStats, PlayerTeamItem,
    ProfilePostItem, ProfilePostsResponse, SimpleStatus, UpdateProfileRequest,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct PostsQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, FromRow)]
struct ProfileRow {
    id: Uuid,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
    bio: String,
    skin_url: Option<String>,
    skin_model: String,
    cape_url: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
    online: bool,
    last_seen_at: Option<DateTime<Utc>>,
}

async fn build_profile(pool: &sqlx::PgPool, row: ProfileRow) -> Result<PlayerProfile, ApiError> {
    let teams = sqlx::query_as::<_, PlayerTeamItem>(
        r#"
        SELECT t.id, t.slug, t.title, tm.role, t.verified
        FROM team_members tm
        JOIN teams t ON t.id = tm.team_id
        WHERE tm.user_id = $1 AND t.status = 'active'
        ORDER BY CASE tm.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, t.title
        "#,
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;

    let modes = sqlx::query_as::<_, PlayerModeItem>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.cover_url, m.status,
               (SELECT COUNT(*) FROM mode_events e
                WHERE e.mode_id = m.id AND e.event = 'install') AS installs
        FROM modes m
        LEFT JOIN mode_authors ma ON ma.mode_id = m.id
        WHERE m.status = 'published'
          AND (m.author_id = $1 OR ma.user_id = $1)
        ORDER BY m.updated_at DESC
        LIMIT 50
        "#,
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;

    let stats = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
        SELECT COUNT(DISTINCT m.id),
               COUNT(e.id) FILTER (WHERE e.event = 'download'),
               COUNT(e.id) FILTER (WHERE e.event = 'install')
        FROM modes m
        LEFT JOIN mode_authors ma ON ma.mode_id = m.id
        LEFT JOIN mode_events e ON e.mode_id = m.id
        WHERE m.status = 'published'
          AND (m.author_id = $1 OR ma.user_id = $1)
        "#,
    )
    .bind(row.id)
    .fetch_one(pool)
    .await?;

    Ok(PlayerProfile {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        avatar_url: row.avatar_url,
        bio: row.bio,
        skin_url: row.skin_url,
        skin_model: row.skin_model,
        cape_url: row.cape_url,
        role: row.role,
        created_at: row.created_at,
        online: row.online,
        last_seen_at: row.last_seen_at,
        teams,
        modes,
        stats: PlayerStats {
            modes: stats.0,
            downloads: stats.1,
            installs: stats.2,
        },
    })
}

pub async fn profile(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<PlayerProfile>, ApiError> {
    let row = sqlx::query_as::<_, ProfileRow>(
        r#"
        SELECT id, username, display_name, avatar_url, bio, skin_url, skin_model,
               cape_url, role, created_at,
               COALESCE(last_seen_at > now() - interval '2 minutes', false) AS online,
               last_seen_at
        FROM users
        WHERE lower(username) = lower($1)
        "#,
    )
    .bind(username.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Игрок не найден".to_string()))?;
    Ok(Json(build_profile(&state.pool, row).await?))
}

pub async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let username = match body.username.trim() {
        "" => user.username.to_ascii_lowercase(),
        value => value.to_ascii_lowercase(),
    };
    if !auth::validate_username(&username) {
        return Err(ApiError::BadRequest(
            "ID профиля: 3–20 символов, только латиница и цифры".to_string(),
        ));
    }
    let display_name = body.display_name.trim();
    if display_name.is_empty() || display_name.chars().count() > 16 {
        return Err(ApiError::BadRequest("Ник: от 1 до 16 символов".to_string()));
    }
    let bio = body.bio.trim();
    if bio.chars().count() > 500 {
        return Err(ApiError::BadRequest(
            "Статус: не больше 500 символов".to_string(),
        ));
    }
    let taken = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM users WHERE lower(username) = lower($1) AND id <> $2)",
    )
    .bind(&username)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if taken {
        return Err(ApiError::Conflict("Этот ID уже занят".to_string()));
    }
    sqlx::query("UPDATE users SET username = $2, display_name = $3, bio = $4, updated_at = now() WHERE id = $1")
        .bind(user.id)
        .bind(&username)
        .bind(display_name)
        .bind(bio)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "updated".to_string(),
    }))
}

async fn fetch_profile_post(pool: &sqlx::PgPool, id: Uuid) -> Result<ProfilePostItem, ApiError> {
    sqlx::query_as::<_, ProfilePostItem>(
        r#"
        SELECT p.id, p.body, p.created_at,
               u.username, u.display_name, u.avatar_url, u.skin_url, u.skin_model
        FROM profile_posts p
        JOIN users u ON u.id = p.user_id
        WHERE p.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Пост не найден".to_string()))
}

pub async fn posts(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(query): Query<PostsQuery>,
) -> Result<Json<ProfilePostsResponse>, ApiError> {
    let user_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE lower(username) = lower($1)")
            .bind(username.trim())
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("Игрок не найден".to_string()))?;
    let offset = query.offset.unwrap_or(0).max(0);
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let total =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM profile_posts WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await?;
    let items = sqlx::query_as::<_, ProfilePostItem>(
        r#"
        SELECT p.id, p.body, p.created_at,
               u.username, u.display_name, u.avatar_url, u.skin_url, u.skin_model
        FROM profile_posts p
        JOIN users u ON u.id = p.user_id
        WHERE p.user_id = $1
        ORDER BY p.created_at DESC
        OFFSET $2 LIMIT $3
        "#,
    )
    .bind(user_id)
    .bind(offset)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(ProfilePostsResponse {
        items,
        total,
        offset,
        limit,
    }))
}

pub async fn create_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateProfilePostRequest>,
) -> Result<Json<ProfilePostItem>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let text = body.body.trim();
    if text.is_empty() || text.chars().count() > 1000 {
        return Err(ApiError::BadRequest(
            "Пост: от 1 до 1000 символов".to_string(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO profile_posts (id, user_id, body) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(user.id)
        .bind(text)
        .execute(&state.pool)
        .await?;
    Ok(Json(fetch_profile_post(&state.pool, id).await?))
}

pub async fn delete_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let result = sqlx::query("DELETE FROM profile_posts WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Пост не найден".to_string()));
    }
    Ok(Json(SimpleStatus {
        ok: true,
        status: "deleted".to_string(),
    }))
}
