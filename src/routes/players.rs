use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    PlayerModeItem, PlayerProfile, PlayerStats, PlayerTeamItem, SimpleStatus, UpdateProfileRequest,
};
use crate::AppState;

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
        SELECT DISTINCT m.id, m.slug, m.title, m.short_description, m.cover_url, m.status,
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
               cape_url, role, created_at
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
    let display_name = body.display_name.trim();
    if display_name.is_empty() || display_name.chars().count() > 48 {
        return Err(ApiError::BadRequest(
            "Отображаемое имя: от 1 до 48 символов".to_string(),
        ));
    }
    let bio = body.bio.trim();
    if bio.chars().count() > 500 {
        return Err(ApiError::BadRequest(
            "Статус: не больше 500 символов".to_string(),
        ));
    }
    sqlx::query("UPDATE users SET display_name = $2, bio = $3, updated_at = now() WHERE id = $1")
        .bind(user.id)
        .bind(display_name)
        .bind(bio)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "updated".to_string(),
    }))
}
