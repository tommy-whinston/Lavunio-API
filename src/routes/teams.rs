use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    SimpleStatus, Team, TeamCard, TeamInvitationItem, TeamInviteRequest, TeamMember,
    TeamMemberRoleRequest, TeamProfile, TeamRequest,
};
use crate::AppState;

fn valid_member_role(role: &str) -> bool {
    matches!(role, "admin" | "publisher" | "designer" | "member")
}

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

pub async fn invitations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TeamInvitationItem>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let items = sqlx::query_as::<_, TeamInvitationItem>(
        r#"
        SELECT i.id, i.team_id, t.slug AS team_slug, t.title AS team_title,
               u.display_name AS inviter_name, i.role, i.created_at
        FROM team_invitations i
        JOIN teams t ON t.id = i.team_id
        JOIN users u ON u.id = i.inviter_id
        WHERE i.invitee_id = $1 AND i.status = 'pending' AND t.status = 'active'
        ORDER BY i.created_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamInviteRequest>,
) -> Result<Json<Team>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let team = fetch_team(&state.pool, team_id).await?;
    if team.status != "active" {
        return Err(ApiError::BadRequest("Команда в архиве".to_string()));
    }
    let actor_role = team_role(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = actor_role.as_deref() == Some("owner");
    let is_team_admin = actor_role.as_deref() == Some("admin");
    if !is_platform_admin && !is_owner && !is_team_admin {
        return Err(ApiError::Forbidden(
            "Приглашать участников могут владелец и администратор команды".to_string(),
        ));
    }

    let role = body.role.trim().to_lowercase();
    if !valid_member_role(&role) {
        return Err(ApiError::BadRequest(
            "role: admin, publisher, designer или member".to_string(),
        ));
    }
    if role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "Назначать администраторов может только владелец".to_string(),
        ));
    }

    let invitee = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, display_name FROM users WHERE lower(username) = lower($1)",
    )
    .bind(body.username.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Пользователь не найден".to_string()))?;
    if invitee.0 == user.id {
        return Err(ApiError::BadRequest(
            "Нельзя пригласить самого себя".to_string(),
        ));
    }

    let member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2)",
    )
    .bind(team_id)
    .bind(invitee.0)
    .fetch_one(&state.pool)
    .await?;
    if member {
        return Err(ApiError::Conflict(
            "Пользователь уже состоит в команде".to_string(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO team_invitations (id, team_id, inviter_id, invitee_id, role)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
    .bind(user.id)
    .bind(invitee.0)
    .bind(&role)
    .execute(&state.pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            ApiError::Conflict("Приглашение уже отправлено".to_string())
        }
        other => ApiError::from(other),
    })?;

    tracing::info!(
        "user {} invited {} to team {}",
        user.username,
        invitee.1,
        team.slug
    );
    Ok(Json(team))
}

pub async fn accept_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(invitation_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let mut tx = state.pool.begin().await?;
    let invitation = sqlx::query_as::<_, (Uuid, Uuid, String, String)>(
        "SELECT team_id, invitee_id, role, status FROM team_invitations WHERE id = $1 FOR UPDATE",
    )
    .bind(invitation_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Приглашение не найдено".to_string()))?;
    if invitation.1 != user.id {
        return Err(ApiError::Forbidden(
            "Это приглашение адресовано другому пользователю".to_string(),
        ));
    }
    if invitation.3 != "pending" {
        return Err(ApiError::Conflict(
            "Приглашение уже обработано".to_string(),
        ));
    }
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(invitation.0)
    .bind(user.id)
    .bind(&invitation.2)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE team_invitations SET status = 'accepted', resolved_at = now() WHERE id = $1",
    )
    .bind(invitation_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "accepted".to_string(),
    }))
}

pub async fn decline_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(invitation_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let updated = sqlx::query(
        "UPDATE team_invitations SET status = 'declined', resolved_at = now()
         WHERE id = $1 AND invitee_id = $2 AND status = 'pending'",
    )
    .bind(invitation_id)
    .bind(user.id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(ApiError::NotFound(
            "Приглашение не найдено или уже обработано".to_string(),
        ));
    }
    Ok(Json(SimpleStatus {
        ok: true,
        status: "declined".to_string(),
    }))
}

pub async fn set_member_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<TeamMemberRoleRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let actor_role = team_role(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = actor_role.as_deref() == Some("owner");
    let is_team_admin = actor_role.as_deref() == Some("admin");
    if !is_platform_admin && !is_owner && !is_team_admin {
        return Err(ApiError::Forbidden(
            "Менять роли могут владелец и администратор команды".to_string(),
        ));
    }

    let role = body.role.trim().to_lowercase();
    if !valid_member_role(&role) {
        return Err(ApiError::BadRequest(
            "role: admin, publisher, designer или member".to_string(),
        ));
    }
    if role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "Назначать администраторов может только владелец".to_string(),
        ));
    }

    let target_role = team_role(&state.pool, team_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Участник не найден".to_string()))?;
    if target_role == "owner" {
        return Err(ApiError::Forbidden(
            "Роль владельца нельзя изменить".to_string(),
        ));
    }
    if target_role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "Администратор не может менять другого администратора".to_string(),
        ));
    }

    sqlx::query("UPDATE team_members SET role = $3 WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(user_id)
        .bind(&role)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "updated".to_string(),
    }))
}

pub async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let actor_role = team_role(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = actor_role.as_deref() == Some("owner");
    let is_team_admin = actor_role.as_deref() == Some("admin");
    let is_self = user.id == user_id;

    if is_self {
        if actor_role.as_deref() == Some("owner") {
            return Err(ApiError::Forbidden(
                "Владелец не может покинуть команду. Сначала передай владение".to_string(),
            ));
        }
    } else if !is_platform_admin && !is_owner && !is_team_admin {
        return Err(ApiError::Forbidden(
            "Удалять участников могут владелец и администратор команды".to_string(),
        ));
    }

    let target_role = team_role(&state.pool, team_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Участник не найден".to_string()))?;
    if target_role == "owner" {
        return Err(ApiError::Forbidden(
            "Владельца нельзя удалить из команды".to_string(),
        ));
    }
    if target_role == "admin" && !is_self && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "Администратор не может удалить другого администратора".to_string(),
        ));
    }

    sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "removed".to_string(),
    }))
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
