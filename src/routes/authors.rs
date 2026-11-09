use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::auth;
use crate::error::ApiError;
use crate::models::{
    AuthorApplication, AuthorApplicationItem, AuthorApplicationRequest,
    RejectAuthorApplicationRequest,
};
use crate::AppState;

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<AuthorApplication>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let application = sqlx::query_as::<_, AuthorApplication>(
        r#"
        SELECT id, message, links, status, note, created_at, reviewed_at
        FROM author_applications
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(application))
}

pub async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AuthorApplicationRequest>,
) -> Result<Json<AuthorApplication>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    if user.role != "player" {
        return Err(ApiError::BadRequest(
            "Заявка доступна только игрокам без доступа к публикации".to_string(),
        ));
    }

    let message = body.message.trim();
    let length = message.chars().count();
    if !(20..=2000).contains(&length) {
        return Err(ApiError::BadRequest(
            "Расскажи о себе: от 20 до 2000 символов".to_string(),
        ));
    }
    let links = body.links.trim();
    if links.chars().count() > 500 {
        return Err(ApiError::BadRequest("Ссылки слишком длинные".to_string()));
    }

    let open = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM author_applications WHERE user_id = $1 AND status = 'open')",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if open {
        return Err(ApiError::Conflict(
            "Заявка уже отправлена и ждёт рассмотрения".to_string(),
        ));
    }

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO author_applications (id, user_id, message, links)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(user.id)
    .bind(message)
    .bind(links)
    .execute(&state.pool)
    .await?;

    let application = sqlx::query_as::<_, AuthorApplication>(
        r#"
        SELECT id, message, links, status, note, created_at, reviewed_at
        FROM author_applications
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    tracing::info!("user {} applied for author access", user.username);
    Ok(Json(application))
}

pub async fn admin_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuthorApplicationItem>>, ApiError> {
    auth::require_admin(&state.pool, &headers).await?;
    let items = sqlx::query_as::<_, AuthorApplicationItem>(
        r#"
        SELECT a.id, a.user_id, u.username, u.display_name,
               a.message, a.links, a.created_at
        FROM author_applications a
        JOIN users u ON u.id = a.user_id
        WHERE a.status = 'open'
        ORDER BY a.created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

pub async fn admin_approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<AuthorApplication>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT user_id, status FROM author_applications WHERE id = $1 FOR UPDATE",
    )
    .bind(application_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Заявка не найдена".to_string()))?;
    let (user_id, status) = row;
    if status != "open" {
        return Err(ApiError::Conflict(
            "Заявка уже рассмотрена".to_string(),
        ));
    }

    sqlx::query("UPDATE users SET role = 'author', updated_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        UPDATE author_applications
        SET status = 'approved', reviewed_at = now(), reviewed_by = $2
        WHERE id = $1
        "#,
    )
    .bind(application_id)
    .bind(admin.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'user', $3, 'author_approve', '')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let application = sqlx::query_as::<_, AuthorApplication>(
        r#"
        SELECT id, message, links, status, note, created_at, reviewed_at
        FROM author_applications
        WHERE id = $1
        "#,
    )
    .bind(application_id)
    .fetch_one(&state.pool)
    .await?;

    tracing::info!(
        "moderator {} approved author application {}",
        admin.username,
        application_id
    );
    Ok(Json(application))
}

pub async fn admin_reject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<RejectAuthorApplicationRequest>,
) -> Result<Json<AuthorApplication>, ApiError> {
    let admin = auth::require_admin(&state.pool, &headers).await?;
    let note = body.note.trim();
    if note.chars().count() < 5 {
        return Err(ApiError::BadRequest(
            "Укажи причину отклонения (минимум 5 символов)".to_string(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT user_id, status FROM author_applications WHERE id = $1 FOR UPDATE",
    )
    .bind(application_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Заявка не найдена".to_string()))?;
    let (user_id, status) = row;
    if status != "open" {
        return Err(ApiError::Conflict(
            "Заявка уже рассмотрена".to_string(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE author_applications
        SET status = 'rejected', note = $2, reviewed_at = now(), reviewed_by = $3
        WHERE id = $1
        "#,
    )
    .bind(application_id)
    .bind(note)
    .bind(admin.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO moderation_actions (id, moderator_id, target_type, target_id, action, note)
        VALUES ($1, $2, 'user', $3, 'author_reject', $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(admin.id)
    .bind(user_id)
    .bind(note)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let application = sqlx::query_as::<_, AuthorApplication>(
        r#"
        SELECT id, message, links, status, note, created_at, reviewed_at
        FROM author_applications
        WHERE id = $1
        "#,
    )
    .bind(application_id)
    .fetch_one(&state.pool)
    .await?;

    tracing::info!(
        "moderator {} rejected author application {}: {}",
        admin.username,
        application_id,
        note
    );
    Ok(Json(application))
}
