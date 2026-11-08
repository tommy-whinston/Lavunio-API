use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use image::ImageFormat;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::ApiError;
use crate::models::{
    Asset, CompleteAssetRequest, CoverResponse, CreateModeRequest, CreateModeResponse,
    CreateReleaseRequest, CreateReleaseResponse, ModeEventRequest, PublisherMode, PublisherModeRow,
    PublisherStats, Release, SimpleStatus, UploadUrlRequest, UploadUrlResponse,
};
use crate::storage::{Storage, StorageKind};
use crate::AppState;

#[derive(Debug, FromRow)]
struct ReleaseOwnerRow {
    author_id: Uuid,
    mode_id: Uuid,
    status: String,
}

#[derive(Debug, FromRow)]
struct AssetUploadRow {
    author_id: Uuid,
    status: String,
    size: i64,
    sha256: Option<String>,
    storage_key: Option<String>,
}

async fn ensure_owner(user: &AuthUser, author_id: Uuid) -> Result<(), ApiError> {
    if user.id != author_id && user.role != "admin" {
        return Err(ApiError::Forbidden(
            "Можно изменять только свои режимы".to_string(),
        ));
    }
    Ok(())
}

fn ensure_author(user: &AuthUser) -> Result<(), ApiError> {
    if !matches!(user.role.as_str(), "author" | "admin") {
        return Err(ApiError::Forbidden(
            "Публикация доступна только авторам".to_string(),
        ));
    }
    Ok(())
}

async fn ensure_mode_owner(pool: &PgPool, user: &AuthUser, mode_id: Uuid) -> Result<(), ApiError> {
    let author_id = sqlx::query_scalar::<_, Uuid>("SELECT author_id FROM modes WHERE id = $1")
        .bind(mode_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))?;
    ensure_owner(user, author_id).await
}

async fn mode_stats(pool: &PgPool, mode_id: Uuid) -> Result<PublisherStats, ApiError> {
    let stats = sqlx::query_as::<_, PublisherStats>(
        r#"
        SELECT COUNT(*) FILTER (WHERE event = 'download') AS downloads,
               COUNT(*) FILTER (WHERE event = 'install') AS installs,
               COUNT(*) FILTER (WHERE event = 'update') AS updates,
               COUNT(DISTINCT user_id) AS unique_users,
               MAX(created_at) AS last_event_at
        FROM mode_events
        WHERE mode_id = $1
        "#,
    )
    .bind(mode_id)
    .fetch_one(pool)
    .await?;
    Ok(stats)
}

async fn release_owner(pool: &PgPool, release_id: Uuid) -> Result<ReleaseOwnerRow, ApiError> {
    sqlx::query_as::<_, ReleaseOwnerRow>(
        r#"
        SELECT m.author_id, m.id AS mode_id, r.status
        FROM releases r
        JOIN modes m ON m.id = r.mode_id
        WHERE r.id = $1
        "#,
    )
    .bind(release_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Релиз не найден".to_string()))
}

async fn set_tags(pool: &PgPool, mode_id: Uuid, tags: &[String]) -> Result<(), ApiError> {
    for raw in tags {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        let slug = crate::slugify(name);
        if slug.is_empty() {
            continue;
        }

        let tag_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO tags (id, slug, name)
            VALUES ($1, $2, $3)
            ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&slug)
        .bind(name)
        .fetch_one(pool)
        .await?;

        sqlx::query(
            "INSERT INTO mode_tags (mode_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(mode_id)
        .bind(tag_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn create_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateModeRequest>,
) -> Result<Json<CreateModeResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    ensure_author(&user)?;
    let slug = body.slug.trim().to_lowercase();
    if !super::validate_slug(&slug) {
        return Err(ApiError::BadRequest(
            "slug: только строчные латинские буквы, цифры и дефис".to_string(),
        ));
    }
    if body.title.trim().is_empty() {
        return Err(ApiError::BadRequest("Укажи название режима".to_string()));
    }
    let trailer_url = body
        .trailer_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(url) = trailer_url.as_deref() {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ApiError::BadRequest(
                "Ссылка на трейлер должна начинаться с http:// или https://".to_string(),
            ));
        }
        if url.chars().count() > 500 {
            return Err(ApiError::BadRequest(
                "Ссылка на трейлер слишком длинная".to_string(),
            ));
        }
    }

    let mode_id = Uuid::new_v4();
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO modes
            (id, slug, title, short_description, description, status, cover_url, trailer_url,
             author_id, mc_version, loader, loader_version, size_label)
        VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(mode_id)
    .bind(&slug)
    .bind(body.title.trim())
    .bind(body.short_description.trim())
    .bind(&body.description)
    .bind(&body.cover_url)
    .bind(trailer_url.as_deref())
    .bind(user.id)
    .bind(body.mc_version.unwrap_or_default().trim())
    .bind(
        body.loader
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("vanilla"),
    )
    .bind(body.loader_version)
    .bind(body.size)
    .fetch_one(&state.pool)
    .await?;

    sqlx::query(
        "INSERT INTO mode_authors (mode_id, user_id, role, sort_order) VALUES ($1, $2, 'author', 0)",
    )
    .bind(inserted)
    .bind(user.id)
    .execute(&state.pool)
    .await?;

    set_tags(&state.pool, inserted, &body.tags).await?;

    tracing::info!("publisher {} created mode {}", user.username, slug);

    Ok(Json(CreateModeResponse {
        id: inserted,
        slug,
        status: "draft".to_string(),
    }))
}

pub async fn my_modes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PublisherMode>>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    ensure_author(&user)?;

    let rows = sqlx::query_as::<_, PublisherModeRow>(
        r#"
        SELECT id, slug, title, short_description, description, status, cover_url,
               mc_version, loader, loader_version, size_label, created_at, updated_at
        FROM modes
        WHERE author_id = $1
        ORDER BY updated_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let mut releases = sqlx::query_as::<_, Release>(
            r#"
            SELECT id, version, status, channel, changelog, published_at, created_at,
                   rejection_note, moderated_at
            FROM releases
            WHERE mode_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(row.id)
        .fetch_all(&state.pool)
        .await?;

        for release in &mut releases {
            release.assets = sqlx::query_as::<_, Asset>(
                r#"
                SELECT id, kind, url, size, sha256, parts, manifest, file_name, storage_key,
                       storage_kind
                FROM release_assets
                WHERE release_id = $1
                ORDER BY created_at
                "#,
            )
            .bind(release.id)
            .fetch_all(&state.pool)
            .await?;
        }

        items.push(PublisherMode {
            id: row.id,
            slug: row.slug,
            title: row.title,
            short_description: row.short_description,
            description: row.description,
            status: row.status,
            cover_url: row.cover_url,
            mc_version: row.mc_version,
            loader: row.loader,
            loader_version: row.loader_version,
            size: row.size_label,
            created_at: row.created_at,
            updated_at: row.updated_at,
            releases,
            stats: mode_stats(&state.pool, row.id).await?,
        });
    }

    Ok(Json(items))
}

pub async fn create_release(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
    Json(body): Json<CreateReleaseRequest>,
) -> Result<Json<CreateReleaseResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let owner = sqlx::query_scalar::<_, Uuid>("SELECT author_id FROM modes WHERE id = $1")
        .bind(mode_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))?;
    ensure_owner(&user, owner).await?;

    let version = body.version.trim();
    if version.is_empty() || version.len() > 32 {
        return Err(ApiError::BadRequest(
            "Укажи корректную версию релиза".to_string(),
        ));
    }
    let channel = body.channel.as_deref().unwrap_or("stable");
    if channel != "stable" && channel != "beta" {
        return Err(ApiError::BadRequest("channel: stable или beta".to_string()));
    }

    let release_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO releases (id, mode_id, version, status, channel, changelog)
        VALUES ($1, $2, $3, 'draft', $4, $5)
        "#,
    )
    .bind(release_id)
    .bind(mode_id)
    .bind(version)
    .bind(channel)
    .bind(&body.changelog)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "publisher {} created release {} for mode {}",
        user.username,
        version,
        mode_id
    );

    Ok(Json(CreateReleaseResponse {
        id: release_id,
        version: version.to_string(),
        status: "draft".to_string(),
    }))
}

pub async fn upload_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(release_id): Path<Uuid>,
    Json(body): Json<UploadUrlRequest>,
) -> Result<Json<UploadUrlResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let release = release_owner(&state.pool, release_id).await?;
    ensure_owner(&user, release.author_id).await?;
    if release.status == "published" {
        return Err(ApiError::Conflict(
            "Опубликованный релиз нельзя изменить".to_string(),
        ));
    }

    let kind = body.kind.trim();
    if !matches!(
        kind,
        "archive" | "cover" | "manifest" | "screenshot" | "other"
    ) {
        return Err(ApiError::BadRequest("Неизвестный тип файла".to_string()));
    }
    if body.file_name.trim().is_empty() {
        return Err(ApiError::BadRequest("Укажи имя файла".to_string()));
    }

    let asset_id = Uuid::new_v4();
    let storage_kind = state.storage.kind();
    let storage_key =
        Storage::asset_key(release.mode_id, release_id, asset_id, body.file_name.trim());

    let upload_url = if storage_kind == StorageKind::S3 {
        state
            .storage
            .presign_put(&storage_key, state.config.s3_presign_expires_secs)
            .await
            .map_err(ApiError::Internal)?
    } else {
        format!(
            "{}/v1/publisher/releases/{}/assets/{}/upload",
            state.config.public_base_url.trim_end_matches('/'),
            release_id,
            asset_id
        )
    };

    let complete_url = if storage_kind == StorageKind::S3 {
        Some(format!(
            "{}/v1/publisher/releases/{}/assets/{}/complete",
            state.config.public_base_url.trim_end_matches('/'),
            release_id,
            asset_id
        ))
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO release_assets
            (id, release_id, kind, url, size, sha256, manifest, file_name, storage_key, storage_kind)
        VALUES ($1, $2, $3, '', $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(asset_id)
    .bind(release_id)
    .bind(kind)
    .bind(body.size.max(0))
    .bind(&body.sha256)
    .bind(&body.manifest)
    .bind(body.file_name.trim())
    .bind(&storage_key)
    .bind(storage_kind.as_str())
    .execute(&state.pool)
    .await?;

    Ok(Json(UploadUrlResponse {
        asset_id,
        upload_url,
        method: "PUT",
        storage: storage_kind.as_str().to_string(),
        complete_url,
    }))
}

pub async fn upload_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((release_id, asset_id)): Path<(Uuid, Uuid)>,
    bytes: Bytes,
) -> Result<Json<Asset>, ApiError> {
    if state.storage.kind() == StorageKind::S3 {
        return Err(ApiError::BadRequest(
            "Для S3 используй pre-signed URL и endpoint complete".to_string(),
        ));
    }

    let user = auth::require_user(&state.pool, &headers).await?;
    let asset = sqlx::query_as::<_, AssetUploadRow>(
        r#"
        SELECT m.author_id, r.status, a.size, a.sha256, a.storage_key
        FROM release_assets a
        JOIN releases r ON r.id = a.release_id
        JOIN modes m ON m.id = r.mode_id
        WHERE a.id = $1 AND a.release_id = $2
        "#,
    )
    .bind(asset_id)
    .bind(release_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Файл не найден".to_string()))?;

    ensure_owner(&user, asset.author_id).await?;
    if asset.status == "published" {
        return Err(ApiError::Conflict(
            "Опубликованный релиз нельзя изменить".to_string(),
        ));
    }
    if asset.size > 0 && bytes.len() as i64 != asset.size {
        return Err(ApiError::BadRequest(format!(
            "Размер файла не совпадает: ожидалось {}, получено {}",
            asset.size,
            bytes.len()
        )));
    }

    let sha256 = hex::encode(Sha256::digest(&bytes));
    if let Some(expected) = asset.sha256.as_deref() {
        if !expected.eq_ignore_ascii_case(&sha256) {
            return Err(ApiError::BadRequest(
                "sha256 файла не совпадает".to_string(),
            ));
        }
    }

    let key = asset
        .storage_key
        .clone()
        .unwrap_or_else(|| asset_id.to_string());
    state
        .storage
        .ensure_local_dir()
        .map_err(ApiError::Internal)?;
    let path = state
        .storage
        .local_path(&key)
        .ok_or_else(|| ApiError::Internal("Локальное хранилище недоступно".to_string()))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| ApiError::Internal(format!("Не удалось создать папку: {error}")))?;
    }
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|error| ApiError::Internal(format!("Не удалось сохранить файл: {error}")))?;

    let url = state.storage.public_url(&key);
    sqlx::query(
        "UPDATE release_assets SET url = $1, size = $2, sha256 = $3, storage_kind = $4 WHERE id = $5",
    )
    .bind(&url)
    .bind(bytes.len() as i64)
    .bind(&sha256)
    .bind(StorageKind::Local.as_str())
    .bind(asset_id)
    .execute(&state.pool)
    .await?;

    let updated = asset_by_id(&state.pool, asset_id).await?;
    Ok(Json(updated))
}

pub async fn complete_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((release_id, asset_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CompleteAssetRequest>,
) -> Result<Json<Asset>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let asset = sqlx::query_as::<_, AssetUploadRow>(
        r#"
        SELECT m.author_id, r.status, a.size, a.sha256, a.storage_key
        FROM release_assets a
        JOIN releases r ON r.id = a.release_id
        JOIN modes m ON m.id = r.mode_id
        WHERE a.id = $1 AND a.release_id = $2
        "#,
    )
    .bind(asset_id)
    .bind(release_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Файл не найден".to_string()))?;

    ensure_owner(&user, asset.author_id).await?;
    if asset.status == "published" {
        return Err(ApiError::Conflict(
            "Опубликованный релиз нельзя изменить".to_string(),
        ));
    }

    if state.storage.kind() == StorageKind::S3 {
        let key = asset
            .storage_key
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("У файла нет storage_key".to_string()))?;
        let size = state
            .storage
            .head_size(key)
            .await
            .map_err(ApiError::Internal)?;
        if let Some(expected) = body.size {
            if expected >= 0 && expected as u64 != size {
                return Err(ApiError::BadRequest(format!(
                    "Размер файла не совпадает: ожидалось {expected}, в S3 {size}"
                )));
            }
        }
        let sha256 = body.sha256.or(asset.sha256);
        let url = state.storage.public_url(key);
        sqlx::query(
            "UPDATE release_assets SET url = $1, size = $2, sha256 = $3, storage_kind = $4 WHERE id = $5",
        )
        .bind(&url)
        .bind(size as i64)
        .bind(&sha256)
        .bind(StorageKind::S3.as_str())
        .bind(asset_id)
        .execute(&state.pool)
        .await?;
    }

    let updated = asset_by_id(&state.pool, asset_id).await?;
    Ok(Json(updated))
}

async fn asset_by_id(pool: &PgPool, asset_id: Uuid) -> Result<Asset, ApiError> {
    sqlx::query_as::<_, Asset>(
        "SELECT id, kind, url, size, sha256, parts, manifest, file_name, storage_key, storage_kind FROM release_assets WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn submit_release(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(release_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let release = release_owner(&state.pool, release_id).await?;
    ensure_owner(&user, release.author_id).await?;
    if release.status != "draft" {
        return Err(ApiError::BadRequest(
            "На модерацию можно отправить только черновик".to_string(),
        ));
    }

    let archives = sqlx::query_as::<_, (Uuid, Option<String>, Option<Value>, Value)>(
        r#"
        SELECT id, storage_key, manifest, parts
        FROM release_assets
        WHERE release_id = $1 AND kind = 'archive' AND url <> ''
        ORDER BY created_at
        LIMIT 1
        "#,
    )
    .bind(release_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::BadRequest("Сначала загрузи сборку режима".to_string()))?;

    let (archive_id, storage_key, manifest, parts) = archives;
    let split = parts
        .as_array()
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    if split {
        tracing::warn!(
            release_id = %release_id,
            "release uses split archive; server-side scan skipped"
        );
    } else {
        let key = storage_key
            .ok_or_else(|| ApiError::BadRequest("У сборки нет файла в хранилище".to_string()))?;
        let bytes = state
            .storage
            .get_bytes(&key)
            .await
            .map_err(ApiError::Internal)?;
        let scan = crate::build_scan::scan_zip(&bytes).map_err(ApiError::BadRequest)?;
        let suspicious = scan
            .get("suspicious")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut updated_manifest = manifest.unwrap_or_else(|| json!({}));
        if let Some(object) = updated_manifest.as_object_mut() {
            object.insert("scan".to_string(), scan);
        }
        sqlx::query("UPDATE release_assets SET manifest = $2 WHERE id = $1")
            .bind(archive_id)
            .bind(&updated_manifest)
            .execute(&state.pool)
            .await?;
        if !suspicious.is_empty() {
            tracing::warn!(
                release_id = %release_id,
                "suspicious files in build: {:?}",
                suspicious
            );
        }
    }

    sqlx::query("UPDATE releases SET status = 'pending' WHERE id = $1")
        .bind(release_id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE modes SET status = 'pending', updated_at = now() WHERE id = $1")
        .bind(release.mode_id)
        .execute(&state.pool)
        .await?;

    tracing::info!("release {} submitted for moderation", release_id);

    Ok(Json(SimpleStatus {
        ok: true,
        status: "pending".to_string(),
    }))
}

pub async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
) -> Result<Json<PublisherStats>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    ensure_mode_owner(&state.pool, &user, mode_id).await?;
    Ok(Json(mode_stats(&state.pool, mode_id).await?))
}

pub async fn record_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(body): Json<ModeEventRequest>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    let event = body.event.trim();
    if !matches!(event, "download" | "install" | "update") {
        return Err(ApiError::BadRequest(
            "event: download, install или update".to_string(),
        ));
    }

    let mode_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM modes WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug.trim())
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Режим не найден".to_string()))?;

    let release_id = if let Some(version) = body
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM releases WHERE mode_id = $1 AND version = $2 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(mode_id)
        .bind(version)
        .fetch_optional(&state.pool)
        .await?
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO mode_events (id, mode_id, user_id, release_id, event) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(mode_id)
    .bind(user.id)
    .bind(release_id)
    .bind(event)
    .execute(&state.pool)
    .await?;

    Ok(Json(SimpleStatus {
        ok: true,
        status: "recorded".to_string(),
    }))
}

pub async fn upload_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
    bytes: Bytes,
) -> Result<Json<CoverResponse>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    ensure_mode_owner(&state.pool, &user, mode_id).await?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(ApiError::BadRequest("Обложка больше 8 МБ".to_string()));
    }

    let format = image::guess_format(&bytes).map_err(|_| {
        ApiError::BadRequest("Не удалось определить формат изображения".to_string())
    })?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(ApiError::BadRequest(
            "Обложка должна быть PNG, JPEG или WebP".to_string(),
        ));
    }
    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| ApiError::BadRequest("Не удалось прочитать изображение".to_string()))?;
    if image.width() > 4096 || image.height() > 4096 {
        return Err(ApiError::BadRequest(
            "Обложка должна быть не больше 4096×4096".to_string(),
        ));
    }

    let output_format = if matches!(format, ImageFormat::Png | ImageFormat::WebP) {
        ImageFormat::Png
    } else {
        ImageFormat::Jpeg
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, output_format)
        .map_err(|_| ApiError::Internal("Не удалось сохранить обложку".to_string()))?;
    let data = cursor.into_inner();
    let hash = hex::encode(Sha256::digest(&data));
    let extension = if output_format == ImageFormat::Png {
        "png"
    } else {
        "jpg"
    };
    let content_type = if output_format == ImageFormat::Png {
        "image/png"
    } else {
        "image/jpeg"
    };
    let key = format!("covers/{mode_id}-{hash}.{extension}");
    state
        .storage
        .put_bytes(&key, &data, Some(content_type))
        .await
        .map_err(ApiError::Internal)?;
    let url = state.storage.public_url(&key);

    sqlx::query("UPDATE modes SET cover_url = $2, updated_at = now() WHERE id = $1")
        .bind(mode_id)
        .bind(&url)
        .execute(&state.pool)
        .await?;

    Ok(Json(CoverResponse { cover_url: url }))
}

pub async fn delete_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(mode_id): Path<Uuid>,
) -> Result<Json<SimpleStatus>, ApiError> {
    let user = auth::require_user(&state.pool, &headers).await?;
    ensure_mode_owner(&state.pool, &user, mode_id).await?;
    sqlx::query("UPDATE modes SET cover_url = NULL, updated_at = now() WHERE id = $1")
        .bind(mode_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(SimpleStatus {
        ok: true,
        status: "cover_deleted".to_string(),
    }))
}
