use axum::body::Bytes;
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
    TeamRole, TeamRoleRequest, TeamStats, TeamTransferRequest, TeamVerifyRequest,
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
        r#"
        SELECT m.role, COALESCE(r.permissions, '{}'::jsonb) || COALESCE(m.permissions, '{}'::jsonb)
        FROM team_members m
        LEFT JOIN team_roles r ON r.team_id = m.team_id AND r.name = m.role
        WHERE m.team_id = $1 AND m.user_id = $2
        "#,
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

async fn role_exists(pool: &sqlx::PgPool, team_id: Uuid, role: &str) -> Result<bool, ApiError> {
    let found = sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM team_roles WHERE team_id = $1 AND name = $2",
    )
    .bind(team_id)
    .bind(role)
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

fn validate_url(value: &Option<String>, label: &str) -> Result<Option<String>, ApiError> {
    let Some(raw) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if !raw.starts_with("http://") && !raw.starts_with("https://") {
        return Err(ApiError::BadRequest(format!(
            "{label} РґРѕР»Р¶РЅР° РЅР°С‡РёРЅР°С‚СЊСЃСЏ СЃ http:// РёР»Рё https://"
        )));
    }
    if raw.chars().count() > 500 {
        return Err(ApiError::BadRequest(format!("{label} СЃР»РёС€РєРѕРј РґР»РёРЅРЅР°СЏ")));
    }
    Ok(Some(raw.to_string()))
}

fn validate_request(body: &TeamRequest) -> Result<(String, String, String, String), ApiError> {
    let slug = body.slug.trim().to_lowercase();
    if !super::validate_slug(&slug) {
        return Err(ApiError::BadRequest(
            "slug: С‚РѕР»СЊРєРѕ СЃС‚СЂРѕС‡РЅС‹Рµ Р»Р°С‚РёРЅСЃРєРёРµ Р±СѓРєРІС‹, С†РёС„СЂС‹ Рё РґРµС„РёСЃ".to_string(),
        ));
    }
    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > 100 {
        return Err(ApiError::BadRequest(
            "РќР°Р·РІР°РЅРёРµ РєРѕРјР°РЅРґС‹: РѕС‚ 1 РґРѕ 100 СЃРёРјРІРѕР»РѕРІ".to_string(),
        ));
    }
    let description = body.description.trim();
    if description.chars().count() > 5000 {
        return Err(ApiError::BadRequest(
            "РћРїРёСЃР°РЅРёРµ РєРѕРјР°РЅРґС‹ СЃР»РёС€РєРѕРј РґР»РёРЅРЅРѕРµ".to_string(),
        ));
    }
    let links = body.links.trim();
    if links.chars().count() > 1000 {
        return Err(ApiError::BadRequest("РЎСЃС‹Р»РєРё СЃР»РёС€РєРѕРј РґР»РёРЅРЅС‹Рµ".to_string()));
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
    .ok_or_else(|| ApiError::NotFound("РљРѕРјР°РЅРґР° РЅРµ РЅР°Р№РґРµРЅР°".to_string()))
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
    let avatar_url = validate_url(&body.avatar_url, "РЎСЃС‹Р»РєР° РЅР° Р°РІР°С‚Р°СЂ")?;
    let banner_url = validate_url(&body.banner_url, "РЎСЃС‹Р»РєР° РЅР° Р±Р°РЅРЅРµСЂ")?;
    let join_mode = body.join_mode.as_deref().unwrap_or("invite").trim();
    if !matches!(join_mode, "invite" | "request" | "open") {
        return Err(ApiError::BadRequest(
            "joinMode: invite, request РёР»Рё open".to_string(),
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
            ApiError::Conflict("РљРѕРјР°РЅРґР° СЃ С‚Р°РєРёРј slug СѓР¶Рµ СЃСѓС‰РµСЃС‚РІСѓРµС‚".to_string())
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
            "РР·РјРµРЅСЏС‚СЊ РєРѕРјР°РЅРґСѓ РјРѕРіСѓС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }
    let (slug, title, description, links) = validate_request(&body)?;
    let avatar_url = validate_url(&body.avatar_url, "РЎСЃС‹Р»РєР° РЅР° Р°РІР°С‚Р°СЂ")?;
    let banner_url = validate_url(&body.banner_url, "РЎСЃС‹Р»РєР° РЅР° Р±Р°РЅРЅРµСЂ")?;
    let join_mode = body.join_mode.as_deref().unwrap_or("invite").trim();
    if !matches!(join_mode, "invite" | "request" | "open") {
        return Err(ApiError::BadRequest(
            "joinMode: invite, request РёР»Рё open".to_string(),
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
            ApiError::Conflict("РљРѕРјР°РЅРґР° СЃ С‚Р°РєРёРј slug СѓР¶Рµ СЃСѓС‰РµСЃС‚РІСѓРµС‚".to_string())
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
            "РђСЂС…РёРІРёСЂРѕРІР°С‚СЊ РєРѕРјР°РЅРґСѓ РјРѕР¶РµС‚ С‚РѕР»СЊРєРѕ РІР»Р°РґРµР»РµС†".to_string(),
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
        return Err(ApiError::BadRequest("РљРѕРјР°РЅРґР° РІ Р°СЂС…РёРІРµ".to_string()));
    }
    let access = member_access(&state.pool, team_id, user.id).await?;
    let is_platform_admin = user.role == "admin";
    let is_owner = access.as_ref().is_some_and(|(role, _)| role == "owner");
    let can_invite = access
        .as_ref()
        .is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_members"));
    if !is_platform_admin && !can_invite {
        return Err(ApiError::Forbidden(
            "РџСЂРёРіР»Р°С€Р°С‚СЊ СѓС‡Р°СЃС‚РЅРёРєРѕРІ РјРѕРіСѓС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }

    let role = body.role.trim().to_lowercase();
    if !role_exists(&state.pool, team_id, &role).await? {
        return Err(ApiError::BadRequest(
            "РќРµРёР·РІРµСЃС‚РЅР°СЏ СЂРѕР»СЊ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }
    if role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "РќР°Р·РЅР°С‡Р°С‚СЊ Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂРѕРІ РјРѕР¶РµС‚ С‚РѕР»СЊРєРѕ РІР»Р°РґРµР»РµС†".to_string(),
        ));
    }

    let invitee = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, display_name FROM users WHERE lower(username) = lower($1)",
    )
    .bind(body.username.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("РџРѕР»СЊР·РѕРІР°С‚РµР»СЊ РЅРµ РЅР°Р№РґРµРЅ".to_string()))?;
    if invitee.0 == user.id {
        return Err(ApiError::BadRequest(
            "РќРµР»СЊР·СЏ РїСЂРёРіР»Р°СЃРёС‚СЊ СЃР°РјРѕРіРѕ СЃРµР±СЏ".to_string(),
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
            "РџРѕР»СЊР·РѕРІР°С‚РµР»СЊ СѓР¶Рµ СЃРѕСЃС‚РѕРёС‚ РІ РєРѕРјР°РЅРґРµ".to_string(),
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
            ApiError::Conflict("РџСЂРёРіР»Р°С€РµРЅРёРµ СѓР¶Рµ РѕС‚РїСЂР°РІР»РµРЅРѕ".to_string())
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
        "РџСЂРёРіР»Р°С€РµРЅРёРµ РІ РєРѕРјР°РЅРґСѓ",
        &format!(
            "{} РїСЂРёРіР»Р°С€Р°РµС‚ С‚РµР±СЏ РІ РєРѕРјР°РЅРґСѓ В«{}В»",
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
    .ok_or_else(|| ApiError::NotFound("РџСЂРёРіР»Р°С€РµРЅРёРµ РЅРµ РЅР°Р№РґРµРЅРѕ".to_string()))?;
    if invitation.1 != user.id {
        return Err(ApiError::Forbidden(
            "Р­С‚Рѕ РїСЂРёРіР»Р°С€РµРЅРёРµ Р°РґСЂРµСЃРѕРІР°РЅРѕ РґСЂСѓРіРѕРјСѓ РїРѕР»СЊР·РѕРІР°С‚РµР»СЋ".to_string(),
        ));
    }
    if invitation.3 != "pending" {
        return Err(ApiError::Conflict("РџСЂРёРіР»Р°С€РµРЅРёРµ СѓР¶Рµ РѕР±СЂР°Р±РѕС‚Р°РЅРѕ".to_string()));
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
            "РџСЂРёРіР»Р°С€РµРЅРёРµ РЅРµ РЅР°Р№РґРµРЅРѕ РёР»Рё СѓР¶Рµ РѕР±СЂР°Р±РѕС‚Р°РЅРѕ".to_string(),
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
            "РњРµРЅСЏС‚СЊ СЂРѕР»Рё РјРѕРіСѓС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }

    let role = body.role.trim().to_lowercase();
    if !role_exists(&state.pool, team_id, &role).await? {
        return Err(ApiError::BadRequest(
            "РќРµРёР·РІРµСЃС‚РЅР°СЏ СЂРѕР»СЊ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }
    if role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "РќР°Р·РЅР°С‡Р°С‚СЊ Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂРѕРІ РјРѕР¶РµС‚ С‚РѕР»СЊРєРѕ РІР»Р°РґРµР»РµС†".to_string(),
        ));
    }
    let permissions = body.permissions.unwrap_or_else(|| json!({}));
    if !permissions.is_object() {
        return Err(ApiError::BadRequest(
            "permissions РґРѕР»Р¶РµРЅ Р±С‹С‚СЊ РѕР±СЉРµРєС‚РѕРј".to_string(),
        ));
    }

    let target_role = team_role(&state.pool, team_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("РЈС‡Р°СЃС‚РЅРёРє РЅРµ РЅР°Р№РґРµРЅ".to_string()))?;
    if target_role == "owner" {
        return Err(ApiError::Forbidden(
            "Р РѕР»СЊ РІР»Р°РґРµР»СЊС†Р° РЅРµР»СЊР·СЏ РёР·РјРµРЅРёС‚СЊ".to_string(),
        ));
    }
    if target_role == "admin" && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "РђРґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РЅРµ РјРѕР¶РµС‚ РјРµРЅСЏС‚СЊ РґСЂСѓРіРѕРіРѕ Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂР°".to_string(),
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
                "Р’Р»Р°РґРµР»РµС† РЅРµ РјРѕР¶РµС‚ РїРѕРєРёРЅСѓС‚СЊ РєРѕРјР°РЅРґСѓ. РЎРЅР°С‡Р°Р»Р° РїРµСЂРµРґР°Р№ РІР»Р°РґРµРЅРёРµ".to_string(),
            ));
        }
    } else if !is_platform_admin && !can_manage {
        return Err(ApiError::Forbidden(
            "РЈРґР°Р»СЏС‚СЊ СѓС‡Р°СЃС‚РЅРёРєРѕРІ РјРѕРіСѓС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }

    let target_role = team_role(&state.pool, team_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("РЈС‡Р°СЃС‚РЅРёРє РЅРµ РЅР°Р№РґРµРЅ".to_string()))?;
    if target_role == "owner" {
        return Err(ApiError::Forbidden(
            "Р’Р»Р°РґРµР»СЊС†Р° РЅРµР»СЊР·СЏ СѓРґР°Р»РёС‚СЊ РёР· РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }
    if target_role == "admin" && !is_self && !is_owner && !is_platform_admin {
        return Err(ApiError::Forbidden(
            "РђРґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РЅРµ РјРѕР¶РµС‚ СѓРґР°Р»РёС‚СЊ РґСЂСѓРіРѕРіРѕ Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂР°".to_string(),
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
        return Err(ApiError::BadRequest("РљРѕРјР°РЅРґР° РІ Р°СЂС…РёРІРµ".to_string()));
    }
    if team.join_mode == "invite" {
        return Err(ApiError::Forbidden(
            "РљРѕРјР°РЅРґР° РїСЂРёРЅРёРјР°РµС‚ С‚РѕР»СЊРєРѕ РїРѕ РїСЂРёРіР»Р°С€РµРЅРёСЏРј".to_string(),
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
        return Err(ApiError::Conflict("РўС‹ СѓР¶Рµ СЃРѕСЃС‚РѕРёС€СЊ РІ РєРѕРјР°РЅРґРµ".to_string()));
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
            "РЎРѕРѕР±С‰РµРЅРёРµ СЃР»РёС€РєРѕРј РґР»РёРЅРЅРѕРµ".to_string(),
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
            ApiError::Conflict("Р—Р°СЏРІРєР° СѓР¶Рµ РѕС‚РїСЂР°РІР»РµРЅР°".to_string())
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
            "Р—Р°СЏРІРєРё РІРёРґСЏС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
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
            "Р Р°СЃСЃРјР°С‚СЂРёРІР°С‚СЊ Р·Р°СЏРІРєРё РјРѕРіСѓС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
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
    .ok_or_else(|| ApiError::NotFound("Р—Р°СЏРІРєР° РЅРµ РЅР°Р№РґРµРЅР°".to_string()))?;
    if row.1 != "pending" {
        return Err(ApiError::Conflict("Р—Р°СЏРІРєР° СѓР¶Рµ СЂР°СЃСЃРјРѕС‚СЂРµРЅР°".to_string()));
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
            "РџРµСЂРµРґР°С‚СЊ РІР»Р°РґРµРЅРёРµ РјРѕР¶РµС‚ С‚РѕР»СЊРєРѕ РІР»Р°РґРµР»РµС†".to_string(),
        ));
    }
    let target = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, display_name FROM users WHERE lower(username) = lower($1)",
    )
    .bind(body.username.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("РџРѕР»СЊР·РѕРІР°С‚РµР»СЊ РЅРµ РЅР°Р№РґРµРЅ".to_string()))?;
    if target.0 == user.id {
        return Err(ApiError::BadRequest("РўС‹ СѓР¶Рµ РІР»Р°РґРµР»РµС† РєРѕРјР°РЅРґС‹".to_string()));
    }
    let target_role = team_role(&state.pool, team_id, target.0)
        .await?
        .ok_or_else(|| ApiError::BadRequest("РџРѕР»СЊР·РѕРІР°С‚РµР»СЊ РЅРµ СЃРѕСЃС‚РѕРёС‚ РІ РєРѕРјР°РЅРґРµ".to_string()))?;
    if target_role == "owner" {
        return Err(ApiError::BadRequest(
            "РџРѕР»СЊР·РѕРІР°С‚РµР»СЊ СѓР¶Рµ РІР»Р°РґРµР»РµС†".to_string(),
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
        return Err(ApiError::BadRequest("РљРѕРјР°РЅРґР° РІ Р°СЂС…РёРІРµ".to_string()));
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
            "РџСѓР±Р»РёРєРѕРІР°С‚СЊ РЅРѕРІРѕСЃС‚Рё РјРѕРіСѓС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string(),
        ));
    }
    let title = body.title.trim();
    let text = body.body.trim();
    if !(3..=120).contains(&title.chars().count()) {
        return Err(ApiError::BadRequest(
            "Р—Р°РіРѕР»РѕРІРѕРє РЅРѕРІРѕСЃС‚Рё: РѕС‚ 3 РґРѕ 120 СЃРёРјРІРѕР»РѕРІ".to_string(),
        ));
    }
    if !(3..=5000).contains(&text.chars().count()) {
        return Err(ApiError::BadRequest(
            "РўРµРєСЃС‚ РЅРѕРІРѕСЃС‚Рё: РѕС‚ 3 РґРѕ 5000 СЃРёРјРІРѕР»РѕРІ".to_string(),
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
    .ok_or_else(|| ApiError::NotFound("РљРѕРјР°РЅРґР° РЅРµ РЅР°Р№РґРµРЅР°".to_string()))?;

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

    let roles = sqlx::query_as::<_, TeamRole>(
        r#"
        SELECT id, name, permissions, position, is_default
        FROM team_roles
        WHERE team_id = $1
        ORDER BY position, name
        "#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

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
        roles,
        role,
        following,
        permissions,
    }))
}

fn validate_role(body: &TeamRoleRequest) -> Result<(String, Value), ApiError> {
    let name = body.name.trim().to_lowercase();
    if name.is_empty() || name.chars().count() > 32 {
        return Err(ApiError::BadRequest("РќР°Р·РІР°РЅРёРµ СЂРѕР»Рё: РѕС‚ 1 РґРѕ 32 СЃРёРјРІРѕР»РѕРІ".to_string()));
    }
    if name == "owner" {
        return Err(ApiError::BadRequest("Р РѕР»СЊ РІР»Р°РґРµР»СЊС†Р° РјРµРЅСЏС‚СЊ РЅРµР»СЊР·СЏ".to_string()));
    }
    let raw = body.permissions.clone().unwrap_or_else(|| json!({}));
    let mut clean = serde_json::Map::new();
    for key in ["manage_team", "manage_members", "manage_modes", "manage_news"] {
        clean.insert(key.to_string(), Value::Bool(raw.get(key).and_then(Value::as_bool).unwrap_or(false)));
    }
    Ok((name, Value::Object(clean)))
}

async fn require_manage_team(state: &AppState, headers: &HeaderMap, team_id: Uuid) -> Result<(), ApiError> {
    let user = auth::require_user(&state.pool, headers).await?;
    let access = member_access(&state.pool, team_id, user.id).await?;
    let allowed = user.role == "admin"
        || access.as_ref().is_some_and(|(role, permissions)| permission_allowed(role, permissions, "manage_team"));
    if !allowed {
        return Err(ApiError::Forbidden("РњРµРЅСЏС‚СЊ СЂРѕР»Рё РјРѕР¶РµС‚ РІР»Р°РґРµР»РµС† Рё Р°РґРјРёРЅРёСЃС‚СЂР°С‚РѕСЂ РєРѕРјР°РЅРґС‹".to_string()));
    }
    Ok(())
}

async fn find_role(state: &AppState, team_id: Uuid, role_id: Uuid) -> Result<TeamRole, ApiError> {
    sqlx::query_as::<_, TeamRole>(
        "SELECT id, name, permissions, position, is_default FROM team_roles WHERE id = $1 AND team_id = $2",
    )
    .bind(role_id)
    .bind(team_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Р РѕР»СЊ РЅРµ РЅР°Р№РґРµРЅР°".to_string()))
}

pub async fn list_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<TeamRole>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let access = member_access(&state.pool, team_id, user.id).await?;
    if user.role != "admin" && access.is_none() {
        return Err(ApiError::Forbidden("Р РѕР»Рё РІРёРґСЏС‚ С‚РѕР»СЊРєРѕ СѓС‡Р°СЃС‚РЅРёРєРё РєРѕРјР°РЅРґС‹".to_string()));
    }
    let roles = sqlx::query_as::<_, TeamRole>(
        "SELECT id, name, permissions, position, is_default FROM team_roles WHERE team_id = $1 ORDER BY position, name",
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(roles))
}

pub async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TeamRoleRequest>,
) -> Result<Json<TeamRole>, ApiError> {
    require_manage_team(&state, &headers, team_id).await?;
    let (name, permissions) = validate_role(&body)?;
    if role_exists(&state.pool, team_id, &name).await? {
        return Err(ApiError::BadRequest("РўР°РєР°СЏ СЂРѕР»СЊ СѓР¶Рµ РµСЃС‚СЊ".to_string()));
    }
    let role = sqlx::query_as::<_, TeamRole>(
        r#"
        INSERT INTO team_roles (id, team_id, name, permissions, position)
        VALUES (
            $1, $2, $3, $4,
            COALESCE((SELECT MAX(position) + 10 FROM team_roles WHERE team_id = $2), 10)
        )
        RETURNING id, name, permissions, position, is_default
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(team_id)
    .bind(&name)
    .bind(&permissions)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(role))
}

pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, role_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<TeamRoleRequest>,
) -> Result<Json<TeamRole>, ApiError> {
    require_manage_team(&state, &headers, team_id).await?;
    let current = find_role(&state, team_id, role_id).await?;
    let (name, permissions) = validate_role(&body)?;
    if current.name != name && role_exists(&state.pool, team_id, &name).await? {
        return Err(ApiError::BadRequest("РўР°РєР°СЏ СЂРѕР»СЊ СѓР¶Рµ РµСЃС‚СЊ".to_string()));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE team_roles SET name = $1, permissions = $2 WHERE id = $3")
        .bind(&name)
        .bind(&permissions)
        .bind(role_id)
        .execute(&mut *tx)
        .await?;
    if current.name != name {
        sqlx::query("UPDATE team_members SET role = $1 WHERE team_id = $2 AND role = $3")
            .bind(&name)
            .bind(team_id)
            .bind(&current.name)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE team_invitations SET role = $1 WHERE team_id = $2 AND role = $3")
            .bind(&name)
            .bind(team_id)
            .bind(&current.name)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(TeamRole {
        name,
        permissions,
        ..current
    }))
}

pub async fn delete_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SimpleStatus>, ApiError> {
    require_manage_team(&state, &headers, team_id).await?;
    let current = find_role(&state, team_id, role_id).await?;
    if current.name == "owner" || current.is_default {
        return Err(ApiError::BadRequest("Р­С‚Сѓ СЂРѕР»СЊ СѓРґР°Р»РёС‚СЊ РЅРµР»СЊР·СЏ".to_string()));
    }
    let used = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_members WHERE team_id = $1 AND role = $2",
    )
    .bind(team_id)
    .bind(&current.name)
    .fetch_one(&state.pool)
    .await?;
    if used > 0 {
        return Err(ApiError::BadRequest("Р РѕР»СЊ РЅР°Р·РЅР°С‡РµРЅР° СѓС‡Р°СЃС‚РЅРёРєР°Рј".to_string()));
    }
    let invited = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_invitations WHERE team_id = $1 AND role = $2 AND status = 'pending'",
    )
    .bind(team_id)
    .bind(&current.name)
    .fetch_one(&state.pool)
    .await?;
    if invited > 0 {
        return Err(ApiError::BadRequest("РќР° СЂРѕР»СЊ РµСЃС‚СЊ РїСЂРёРіР»Р°С€РµРЅРёСЏ".to_string()));
    }
    sqlx::query("DELETE FROM team_roles WHERE id = $1 AND team_id = $2")
        .bind(role_id)
        .bind(team_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "deleted".to_string(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamImageQuery {
    kind: Option<String>,
}

pub async fn upload_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<Uuid>,
    Query(query): Query<TeamImageQuery>,
    bytes: Bytes,
) -> Result<Json<Value>, ApiError> {
    require_manage_team(&state, &headers, team_id).await?;
    let kind = query.kind.as_deref().unwrap_or("avatar").trim();
    if !matches!(kind, "avatar" | "banner") {
        return Err(ApiError::BadRequest("kind: avatar или banner".to_string()));
    }
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err(ApiError::BadRequest(
            "Файл должен быть не больше 2 МБ".to_string(),
        ));
    }
    let (ext, content_type) = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        ("png", "image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        ("jpg", "image/jpeg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        ("webp", "image/webp")
    } else {
        return Err(ApiError::BadRequest(
            "Поддерживаются PNG, JPEG и WebP".to_string(),
        ));
    };
    let key = format!("teams/{team_id}/{kind}-{}.{ext}", Uuid::new_v4());
    state
        .storage
        .put_bytes(&key, &bytes, Some(content_type))
        .await
        .map_err(ApiError::Internal)?;
    let url = state.storage.public_url(&key);
    let column = if kind == "avatar" { "avatar_url" } else { "banner_url" };
    sqlx::query(&format!(
        "UPDATE teams SET {column} = $2, updated_at = now() WHERE id = $1"
    ))
    .bind(team_id)
    .bind(&url)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "url": url })))
}
