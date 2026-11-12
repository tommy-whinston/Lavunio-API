use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    SimpleStatus, Team, TeamCard, TeamDirectoryItem, TeamDirectoryResponse, TeamInvitationItem,
    TeamInviteRequest, TeamJoinRequest, TeamJoinRequestItem, TeamJoinResolveRequest, TeamMember,
    TeamMemberRoleRequest, TeamModeItem, TeamNewsItem, TeamNewsRequest, TeamProfile, TeamRequest,
    TeamStats, TeamTransferRequest, TeamVerifyRequest,
};
use crate::routes::notifications;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TeamsQuery {
    q: Option<String>,
    sort: Option<String>,
    filter: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

fn valid_member_role(role: &str) -> bool {
    matches!(
        role,
        "admin" | "publisher" | "designer" | "builder" | "modeler" | "member"
    )
}

fn role_default_permissions(role: &str) -> Value {
    match role {
        "owner" => json!({
            "manage_team": true,
            "manage_members": true,
            "manage_modes": true,
            "manage_news": true
        }),
        "admin" => json!({
            "manage_team": true,
            "manage_members": true,
            "manage_modes": false,
            "manage_news": true
        }),
        "publisher" | "builder" | "modeler" => json!({
            "manage_team": false,
            "manage_members": false,
            "manage_modes": true,
            "manage_news": false
        }),
        "designer" => json!({
            "manage_team": false,
            "manage_members": false,
            "manage_modes": true,
            "manage_news": false
        }),
        _ => json!({}),
    }
}

fn permission_allowed(role: &str, permissions: &Value, key: &str) -> bool {
    if role == "owner" {
        return true;
    }
    if let Some(value) = permissions.get(key).and_then(Value::as_bool) {
        return value;
    }
    role_default_permissions(role)
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

async fn member_access(
    pool: &sqlx::PgPool,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<Option<(String, Value)>, ApiError> {
    Ok(sqlx::query_as::<_, (String, Value)>(
        "SELECT role, permissions FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
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
               owner_id, status, join_mode, verified, created_at, updated_at
        FROM teams
        WHERE id = $1
        "#,
    )
    .bind(team_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Команда не найдена".to_string()))
}

async fn team_role(
    pool: &sqlx::PgPool,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<Option<String>, ApiError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TeamsQuery>,
) -> Result<Json<TeamDirectoryResponse>, ApiError> {
    let user = auth::optional_user(&state.pool, &headers).await;
    let user_id = user.as_ref().map(|value| value.id);
    let search = query.q.unwrap_or_default().trim().to_lowercase();
    let filter = match query.filter.as_deref().unwrap_or("all") {
        "mine" => "mine",
        "following" => "following",
        _ => "all",
    };
    let order = match query.sort.as_deref().unwrap_or("popular") {
        "new" => "t.created_at DESC",
        "name" => "lower(t.title) ASC",
        _ => "followers DESC, downloads DESC, members_count DESC, t.created_at DESC",
    };
    let offset = query.offset.unwrap_or(0).max(0);
    let limit = query.limit.unwrap_or(24).clamp(1, 60);
    let filter_sql = r#"
        t.status = 'active'
        AND ($1::text = '' OR lower(t.title) LIKE '%' || $1 || '%' OR lower(t.slug) LIKE '%' || $1 || '%')
        AND (
            $2::uuid IS NULL OR $3::text = 'all'
            OR ($3::text = 'mine' AND EXISTS (
                SELECT 1 FROM team_members tm WHERE tm.team_id = t.id AND tm.user_id = $2
            ))
            OR ($3::text = 'following' AND EXISTS (
                SELECT 1 FROM team_followers f WHERE f.team_id = t.id AND f.user_id = $2
            ))
        )
    "#;
    let total =
        sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM teams t WHERE {filter_sql}"))
            .bind(&search)
            .bind(user_id)
            .bind(filter)
            .fetch_one(&state.pool)
            .await?;
    let sql = format!(
        r#"
        SELECT t.id, t.slug, t.title, t.description, t.avatar_url, t.banner_url,
               t.verified, t.join_mode,
               (SELECT COUNT(*) FROM team_members tm WHERE tm.team_id = t.id) AS members_count,
               (SELECT COUNT(*) FROM modes m WHERE m.team_id = t.id AND m.status = 'published') AS modes_count,
               COALESCE((SELECT COUNT(*) FROM mode_events e JOIN modes m ON m.id = e.mode_id
                         WHERE m.team_id = t.id AND e.event = 'download'), 0) AS downloads,
               (SELECT COUNT(*) FROM team_followers f WHERE f.team_id = t.id) AS followers,
               (SELECT tm.role FROM team_members tm WHERE tm.team_id = t.id AND tm.user_id = $2) AS role,
               EXISTS(SELECT 1 FROM team_followers f WHERE f.team_id = t.id AND f.user_id = $2) AS following,
               COALESCE((SELECT tm.permissions FROM team_members tm
                         WHERE tm.team_id = t.id AND tm.user_id = $2), '{{}}'::jsonb) AS permissions,
               t.created_at
        FROM teams t
        WHERE {filter_sql}
        ORDER BY {order}
        OFFSET $4 LIMIT $5
        "#
    );
    let items = sqlx::query_as::<_, TeamDirectoryItem>(&sql)
        .bind(&search)
        .bind(user_id)
        .bind(filter)
        .bind(offset)
        .bind(limit)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(TeamDirectoryResponse { items, total }))
}

pub async fn mine(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TeamCard>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let teams = sqlx::query_as::<_, TeamCard>(
        r#"
        SELECT t.id, t.slug, t.title, t.description, t.links, t.status, t.avatar_url,
               t.banner_url, tm.role, t.verified, t.join_mode,
               (SELECT count(*) FROM modes m WHERE m.team_id = t.id AND m.status = 'published') AS modes_count,
               (SELECT count(*) FROM mode_events e JOIN modes m ON m.id = e.mode_id
                WHERE m.team_id = t.id AND e.event = 'download') AS downloads,
               t.updated_at
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
    let (slug, title, description, links) = validate_request(&body)?;
    let avatar_url = validate_url(&body.avatar_url, "Ссылка на аватар")?;
    let banner_url = validate_url(&body.banner_url, "Ссылка на баннер")?;
    let join_mode = body.join_mode.as_deref().unwrap_or("invite").trim();
    if !matches!(join_mode, "invite" | "request" | "open") {
        return Err(ApiError::BadRequest(
            "joinMode: invite, request или open".to_string(),
        ));
    }

    let id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO teams
            (id, slug, title, description, links, avatar_url, banner_url, owner_id, join_mode)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
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
    .bind(join_mode)
    .execute(&mut *tx)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            ApiError::Conflict("Команда с таким slug уже существует".to_string())
        }
        other => ApiError::from(other),
    })?;
    sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'owner')")
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
    let access = member_access(&state.pool, team_id, user.id).await?;
    let allowed = user.role == "admin"
        || access.as_ref().is_some_and(|(role, permissions)| {
            permission_allowed(role, permissions, "manage_team")
        });
    if !allowed {
        return Err(ApiError::Forbidden(
            "Изменять команду могут владелец и администратор команды".to_string(),
        ));
    }
    let (slug, title, description, links) = validate_request(&body)?;
    let avatar_url = validate_url(&body.avatar_url, "Ссылка на аватар")?;
    let banner_url = validate_url(&body.banner_url, "Ссылка на баннер")?;
    let join_mode = body.join_mode.as_deref().unwrap_or("invite").trim();
    if !matches!(join_mode, "invite" | "request" | "open") {
        return Err(ApiError::BadRequest(
            "joinMode: invite, request или open".to_string(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE teams
        SET slug = $2, title = $3, description = $4, links = $5,
            avatar_url = $6, banner_url = $7, join_mode = $8, updated_at = now()
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
    .bind(join_mode)
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
    let access = member_access(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = access.as_ref().is_some_and(|(role, _)| role == "owner");
    let can_invite = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_members"));
    if !is_platform_admin && !can_invite {
        return Err(ApiError::Forbidden(
            "Приглашать участников могут владелец и администратор команды".to_string(),
        ));
    }

    let role = body.role.trim().to_lowercase();
    if !valid_member_role(&role) {
        return Err(ApiError::BadRequest(
            "role: admin, publisher, designer, builder, modeler или member".to_string(),
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
    notifications::create(
        &state.pool,
        invitee.0,
        "team_invite",
        "Приглашение в команду",
        &format!(
            "{} приглашает тебя в команду «{}»",
            user.display_name, team.title
        ),
        Some("team"),
        Some(&team.slug),
    )
    .await;
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
        return Err(ApiError::Conflict("Приглашение уже обработано".to_string()));
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
    let access = member_access(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = access.as_ref().is_some_and(|(role, _)| role == "owner");
    let can_manage = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_members"));
    if !is_platform_admin && !can_manage {
        return Err(ApiError::Forbidden(
            "Менять роли могут владелец и администратор команды".to_string(),
        ));
    }

    let role = body.role.trim().to_lowercase();
    if !valid_member_role(&role) {
        return Err(ApiError::BadRequest(
            "role: admin, publisher, designer, builder, modeler или member".to_string(),
        ));
    }
    if role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "Назначать администраторов может только владелец".to_string(),
        ));
    }
    let permissions = body.permissions.unwrap_or_else(|| json!({}));
    if !permissions.is_object() {
        return Err(ApiError::BadRequest(
            "permissions должен быть объектом".to_string(),
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

    sqlx::query(
        "UPDATE team_members SET role = $3, permissions = $4 WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .bind(&role)
    .bind(&permissions)
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
    let access = member_access(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = access.as_ref().is_some_and(|(role, _)| role == "owner");
    let can_manage = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_members"));
    let is_self = user.id == user_id;

    if is_self {
        if is_owner {
            return Err(ApiError::Forbidden(
                "Владелец не может покинуть команду. Сначала передай владение".to_string(),
            ));
        }
    } else if !is_platform_admin && !can_manage {
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

pub async fn request_join(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamJoinRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let team = fetch_team(&state.pool, team_id).await?;
    if team.status != "active" {
        return Err(ApiError::BadRequest("Команда в архиве".to_string()));
    }
    if team.join_mode == "invite" {
        return Err(ApiError::Forbidden(
            "Команда принимает только по приглашениям".to_string(),
        ));
    }
    let member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2)",
    )
    .bind(team_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if member {
        return Err(ApiError::Conflict("Ты уже состоишь в команде".to_string()));
    }

    if team.join_mode == "open" {
        sqlx::query(
            "INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
        )
        .bind(team_id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
        return Ok(Json(SimpleStatus {
            ok: true,
            status: "joined".to_string(),
        }));
    }

    let message = body.message.trim();
    if message.chars().count() > 1000 {
        return Err(ApiError::BadRequest(
            "Сообщение слишком длинное".to_string(),
        ));
    }
    sqlx::query(
        "INSERT INTO team_join_requests (id, team_id, user_id, message) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
    .bind(user.id)
    .bind(message)
    .execute(&state.pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            ApiError::Conflict("Заявка уже отправлена".to_string())
        }
        other => ApiError::from(other),
    })?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "pending".to_string(),
    }))
}

pub async fn join_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<TeamJoinRequestItem>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let access = member_access(&state.pool, team_id, user.id).await?;
    let can_manage = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_members"));
    if user.role != "admin" && !can_manage {
        return Err(ApiError::Forbidden(
            "Заявки видят владелец и администратор команды".to_string(),
        ));
    }
    let items = sqlx::query_as::<_, TeamJoinRequestItem>(
        r#"
        SELECT r.id, r.user_id, u.username, u.display_name, r.message, r.created_at
        FROM team_join_requests r
        JOIN users u ON u.id = r.user_id
        WHERE r.team_id = $1 AND r.status = 'pending'
        ORDER BY r.created_at
        "#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn resolve_join_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, request_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<TeamJoinResolveRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let access = member_access(&state.pool, team_id, user.id).await?;
    let can_manage = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_members"));
    if user.role != "admin" && !can_manage {
        return Err(ApiError::Forbidden(
            "Рассматривать заявки могут владелец и администратор команды".to_string(),
        ));
    }
    let accept = body.action.trim() == "accept";
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT user_id, status FROM team_join_requests WHERE id = $1 AND team_id = $2 FOR UPDATE",
    )
    .bind(request_id)
    .bind(team_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Заявка не найдена".to_string()))?;
    if row.1 != "pending" {
        return Err(ApiError::Conflict("Заявка уже рассмотрена".to_string()));
    }
    if accept {
        sqlx::query(
            "INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
        )
        .bind(team_id)
        .bind(row.0)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "UPDATE team_join_requests SET status = $2, resolved_at = now(), resolved_by = $3 WHERE id = $1",
    )
    .bind(request_id)
    .bind(if accept { "accepted" } else { "rejected" })
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: if accept { "accepted" } else { "rejected" }.to_string(),
    }))
}

pub async fn transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamTransferRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let role = team_role(&state.pool, team_id, user.id).await?;
    if role.as_deref() != Some("owner") {
        return Err(ApiError::Forbidden(
            "Передать владение может только владелец".to_string(),
        ));
    }
    let target = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, display_name FROM users WHERE lower(username) = lower($1)",
    )
    .bind(body.username.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Пользователь не найден".to_string()))?;
    if target.0 == user.id {
        return Err(ApiError::BadRequest("Ты уже владелец команды".to_string()));
    }
    let target_role = team_role(&state.pool, team_id, target.0)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Пользователь не состоит в команде".to_string()))?;
    if target_role == "owner" {
        return Err(ApiError::BadRequest(
            "Пользователь уже владелец".to_string(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE team_members SET role = 'admin' WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE team_members SET role = 'owner' WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(target.0)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE teams SET owner_id = $2, updated_at = now() WHERE id = $1")
        .bind(team_id)
        .bind(target.0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!(
        "team {} ownership transferred from {} to {}",
        team_id,
        user.username,
        target.1
    );
    Ok(Json(SimpleStatus {
        ok: true,
        status: "transferred".to_string(),
    }))
}

pub async fn follow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let team = fetch_team(&state.pool, team_id).await?;
    if team.status != "active" {
        return Err(ApiError::BadRequest("Команда в архиве".to_string()));
    }
    sqlx::query(
        "INSERT INTO team_followers (team_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(team_id)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "following".to_string(),
    }))
}

pub async fn unfollow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    sqlx::query("DELETE FROM team_followers WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "unfollowed".to_string(),
    }))
}

pub async fn create_news(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamNewsRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let access = member_access(&state.pool, team_id, user.id).await?;
    let can_manage = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_news"));
    if user.role != "admin" && !can_manage {
        return Err(ApiError::Forbidden(
            "Публиковать новости могут владелец и администратор команды".to_string(),
        ));
    }
    let title = body.title.trim();
    let text = body.body.trim();
    if !(3..=120).contains(&title.chars().count()) {
        return Err(ApiError::BadRequest(
            "Заголовок новости: от 3 до 120 символов".to_string(),
        ));
    }
    if !(3..=5000).contains(&text.chars().count()) {
        return Err(ApiError::BadRequest(
            "Текст новости: от 3 до 5000 символов".to_string(),
        ));
    }
    sqlx::query(
        "INSERT INTO team_news (id, team_id, author_id, title, body) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
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

pub async fn admin_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamVerifyRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    sqlx::query("UPDATE teams SET verified = $2, updated_at = now() WHERE id = $1")
        .bind(team_id)
        .bind(body.verified)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: if body.verified {
            "verified"
        } else {
            "unverified"
        }
        .to_string(),
    }))
}

pub async fn profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Json<TeamProfile>, ApiError> {
    let team = sqlx::query_as::<_, Team>(
        r#"
        SELECT id, slug, title, description, links, avatar_url, banner_url,
               owner_id, status, join_mode, verified, created_at, updated_at
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
        SELECT tm.user_id, u.username, u.display_name, u.avatar_url, tm.role, tm.permissions
        FROM team_members tm
        JOIN users u ON u.id = tm.user_id
        WHERE tm.team_id = $1
        ORDER BY CASE tm.role
            WHEN 'owner' THEN 0
            WHEN 'admin' THEN 1
            WHEN 'publisher' THEN 2
            WHEN 'designer' THEN 3
            WHEN 'builder' THEN 4
            WHEN 'modeler' THEN 5
            ELSE 6
        END, tm.created_at
        "#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    let modes = sqlx::query_as::<_, TeamModeItem>(
        r#"
        SELECT m.id, m.slug, m.title, m.short_description, m.status, m.cover_url,
               (SELECT COUNT(*) FROM mode_events e
                WHERE e.mode_id = m.id AND e.event = 'install') AS installs
        FROM modes m
        WHERE m.team_id = $1 AND m.status = 'published'
        ORDER BY m.updated_at DESC
        "#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    let stats = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM modes m WHERE m.team_id = $1) AS modes,
            (SELECT COUNT(*) FROM modes m WHERE m.team_id = $1 AND m.status = 'published') AS published,
            (SELECT COUNT(*) FROM mode_events e JOIN modes m ON m.id = e.mode_id
             WHERE m.team_id = $1 AND e.event = 'download') AS downloads,
            (SELECT COUNT(*) FROM mode_events e JOIN modes m ON m.id = e.mode_id
             WHERE m.team_id = $1 AND e.event = 'install') AS installs
        "#,
    )
    .bind(team.id)
    .fetch_one(&state.pool)
    .await?;

    let followers =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_followers WHERE team_id = $1")
            .bind(team.id)
            .fetch_one(&state.pool)
            .await?;

    let news = sqlx::query_as::<_, TeamNewsItem>(
        r#"
        SELECT id, title, body, created_at
        FROM team_news
        WHERE team_id = $1 AND status = 'published'
        ORDER BY created_at DESC
        LIMIT 5
        "#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    let viewer = auth::optional_user(&state.pool, &headers).await;
    let (role, permissions, following) = if let Some(user) = viewer {
        let access = member_access(&state.pool, team.id, user.id).await?;
        let following = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM team_followers WHERE team_id = $1 AND user_id = $2)",
        )
        .bind(team.id)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
        match access {
            Some((role, permissions)) => (Some(role), permissions, following),
            None => (None, json!({}), following),
        }
    } else {
        (None, json!({}), false)
    };

    Ok(Json(TeamProfile {
        team,
        members,
        modes,
        stats: TeamStats {
            modes: stats.0,
            published: stats.1,
            downloads: stats.2,
            installs: stats.3,
        },
        followers,
        news,
        role,
        following,
        permissions,
    }))
}
