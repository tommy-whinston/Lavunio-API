use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{Team, TeamCard, TeamMember, TeamProfile, TeamRequest};
use crate::AppState;

fn validate_url(value: &Option<String>, label: &str) -> Result<Option<String>, ApiError> {
    let Some(raw) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if !raw.starts_with("http://") && !raw.starts_with("https://") {
        return Err(ApiError::BadRequest(format!(
            "{label} должна начинаться с http:// или https://"
        )));
    }
    if raw.chars().count() > 500 {
        return Err(ApiError::BadRequest(format!("{label} слишком длинная")));
    }
    Ok(Some(raw.to_string()))
}

fn validate_request(body: &TeamRequest) -> Result<(String, String, String, String), ApiError> {
    let slug = body.slug.trim().to_lowercase();
    if !super::validate_slug(&slug) {
        return Err(ApiError::BadRequest(
            "slug: только строчные латинские буквы, цифры и дефис".to_string(),
        ));
    }
    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > 100 {
        return Err(ApiError::BadRequest(
            "Название команды: от 1 до 100 символов".to_string(),
        ));
    }
    let description = body.description.trim();
    if description.chars().count() > 5000 {
        return Err(ApiError::BadRequest(
            "Описание команды слишком длинное".to_string(),
        ));
    }
    let links = body.links.trim();
    if links.chars().count() > 1000 {
        return Err(ApiError::BadRequest("Ссылки слишком длинные".to_string()));
    }
    Ok((
        slug,
        title.to_string(),
        description.to_string(),
        links.to_string(),
    ))
}

async fn fetch_team(pool: &sqlx::PgPool, team_id: Uuid) -> Result<Team, ApiError> {
    sqlx::query_as::<_, Team>(
        r#"
        SELECT id, slug, title, description, links, avatar_url, banner_url,
               owner_id, status, created_at, updated_at
        FROM teams
        WHERE id = $1
        "#,
    )
    .bind(team_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Команда не найдена".to_string()))
}

async fn team_role(pool: &sqlx::PgPool, team_id: Uuid, user_id: Uuid) -> Result<Option<String>, ApiError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn mine(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TeamCard>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let teams = sqlx::query_as::<_, TeamCard>(
        r#"
        SELECT t.id, t.slug, t.title, t.description, t.links, t.status, t.avatar_url,
               t.banner_url, tm.role, t.updated_at
        FROM teams t
        JOIN team_members tm ON tm.team_id = t.id
        WHERE tm.user_id = $1 AND t.status = 'active'
        ORDER BY t.updated_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(teams))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TeamRequest>,
) -> Result<Json<Team>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    if !matches!(user.role.as_str(), "author" | "admin") {
        return Err(ApiError::Forbidden(
            "Создавать команды могут только авторы".to_string(),
        ));
    }
    let (slug, title, description, links) = validate_request(&body)?;
    let avatar_url = validate_url(&body.avatar_url, "Ссылка на аватар")?;
    let banner_url = validate_url(&body.banner_url, "Ссылка на баннер")?;

    let id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO teams
            (id, slug, title, description, links, avatar_url, banner_url, owner_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(&slug)
    .bind(&title)
    .bind(&description)
    .bind(&links)
    .bind(&avatar_url)
    .bind(&banner_url)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            ApiError::Conflict("Команда с таким slug уже существует".to_string())
        }
        other => ApiError::from(other),
    })?;
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let team = fetch_team(&state.pool, id).await?;
    tracing::info!("user {} created team {}", user.username, slug);
    Ok(Json(team))
}

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamRequest>,
) -> Result<Json<Team>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let role = team_role(&state.pool, team_id, user.id).await?;
    let allowed = user.role == "admin" || matches!(role.as_deref(), Some("owner") | Some("admin"));
    if !allowed {
        return Err(ApiError::Forbidden(
            "Изменять команду могут владелец и администратор команды".to_string(),
        ));
    }
    let (slug, title, description, links) = validate_request(&body)?;
    let avatar_url = validate_url(&body.avatar_url, "Ссылка на аватар")?;
    let banner_url = validate_url(&body.banner_url, "Ссылка на баннер")?;

    sqlx::query(
        r#"
        UPDATE teams
        SET slug = $2, title = $3, description = $4, links = $5,
            avatar_url = $6, banner_url = $7, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(team_id)
    .bind(&slug)
    .bind(&title)
    .bind(&description)
    .bind(&links)
    .bind(&avatar_url)
    .bind(&banner_url)
    .execute(&state.pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            ApiError::Conflict("Команда с таким slug уже существует".to_string())
        }
        other => ApiError::from(other),
    })?;

    let team = fetch_team(&state.pool, team_id).await?;
    tracing::info!("user {} updated team {}", user.username, team.slug);
    Ok(Json(team))
}

pub async fn archive(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Team>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let role = team_role(&state.pool, team_id, user.id).await?;
    let allowed = user.role == "admin" || role.as_deref() == Some("owner");
    if !allowed {
        return Err(ApiError::Forbidden(
            "Архивировать команду может только владелец".to_string(),
        ));
    }
    sqlx::query("UPDATE teams SET status = 'archived', updated_at = now() WHERE id = $1")
        .bind(team_id)
        .execute(&state.pool)
        .await?;
    let team = fetch_team(&state.pool, team_id).await?;
    tracing::info!("user {} archived team {}", user.username, team.slug);
    Ok(Json(team))
}

pub async fn profile(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<TeamProfile>, ApiError> {
    let team = sqlx::query_as::<_, Team>(
        r#"
        SELECT id, slug, title, description, links, avatar_url, banner_url,
               owner_id, status, created_at, updated_at
        FROM teams
        WHERE slug = $1 AND status = 'active'
        "#,
    )
    .bind(slug.trim().to_lowercase())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Команда не найдена".to_string()))?;

    let members = sqlx::query_as::<_, TeamMember>(
        r#"
        SELECT tm.user_id, u.username, u.display_name, u.avatar_url, tm.role
        FROM team_members tm
        JOIN users u ON u.id = tm.user_id
        WHERE tm.team_id = $1
        ORDER BY CASE tm.role
            WHEN 'owner' THEN 0
            WHEN 'admin' THEN 1
            WHEN 'publisher' THEN 2
            WHEN 'designer' THEN 3
            ELSE 4
        END, tm.created_at
        "#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(TeamProfile { team, members }))
}
