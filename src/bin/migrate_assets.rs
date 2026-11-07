use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lavunio_api::config::Config;
use lavunio_api::db;
use lavunio_api::storage::{Storage, StorageKind};

#[derive(Debug, FromRow)]
struct AssetRow {
    id: Uuid,
    release_id: Uuid,
    mode_id: Uuid,
    kind: String,
    url: String,
    size: i64,
    sha256: Option<String>,
    parts: Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let pool = db::pool(&config.database_url);
    db::migrate(&pool).await;
    let storage = Storage::new(&config)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()?;

    let assets = sqlx::query_as::<_, AssetRow>(
        r#"
        SELECT a.id, a.release_id, m.id AS mode_id, a.kind, a.url, a.size, a.sha256, a.parts
        FROM release_assets a
        JOIN releases r ON r.id = a.release_id
        JOIN modes m ON m.id = r.mode_id
        WHERE a.url LIKE 'https://raw.githubusercontent.com/%'
        ORDER BY m.slug, a.kind
        "#,
    )
    .fetch_all(&pool)
    .await?;

    println!("assets to migrate: {}", assets.len());
    for asset in assets {
        migrate_asset(&pool, &storage, &client, &asset).await?;
    }
    println!("asset migration complete");
    Ok(())
}

async fn migrate_asset(
    pool: &PgPool,
    storage: &Storage,
    client: &reqwest::Client,
    asset: &AssetRow,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_name = file_name_from_url(&asset.url, &asset.kind);
    let main_bytes = download(client, &asset.url).await?;
    let main_sha = sha256_hex(&main_bytes);
    let mut parts_json = asset.parts.clone();
    let mut main_url = storage.public_url(&Storage::asset_key(
        asset.mode_id,
        asset.release_id,
        asset.id,
        &file_name,
    ));
    let mut main_size = main_bytes.len() as i64;

    let part_list = asset.parts.as_array().cloned().unwrap_or_default();
    let split_archive = !part_list.is_empty();
    if split_archive {
        let base = asset
            .url
            .rsplit_once('/')
            .map(|(base, _)| base)
            .unwrap_or(&asset.url)
            .to_string();
        let mut migrated_parts = Vec::new();
        for (index, part) in part_list.iter().enumerate() {
            let path = part["path"].as_str().unwrap_or("part");
            let bytes = if index == 0 && path == file_name {
                main_bytes.clone()
            } else {
                download(client, &format!("{base}/{path}")).await?
            };
            let key = Storage::asset_key(asset.mode_id, asset.release_id, asset.id, path);
            storage
                .put_bytes(&key, &bytes, Some(content_type(path)))
                .await?;
            let url = storage.public_url(&key);
            if index == 0 {
                main_url = url.clone();
                main_size = bytes.len() as i64;
            }
            migrated_parts.push(json!({
                "path": path,
                "url": url,
                "size": bytes.len(),
                "sha256": sha256_hex(&bytes)
            }));
        }
        parts_json = Value::Array(migrated_parts);
    } else {
        let key = Storage::asset_key(asset.mode_id, asset.release_id, asset.id, &file_name);
        storage
            .put_bytes(&key, &main_bytes, Some(content_type(&file_name)))
            .await?;
        main_url = storage.public_url(&key);
    }

    let storage_kind = storage.kind().as_str();
    sqlx::query(
        r#"
        UPDATE release_assets
        SET url = $1, size = $2, sha256 = $3, file_name = $4,
            storage_key = $5, storage_kind = $6, parts = $7
        WHERE id = $8
        "#,
    )
    .bind(&main_url)
    .bind(main_size)
    .bind(&main_sha)
    .bind(&file_name)
    .bind(Storage::asset_key(
        asset.mode_id,
        asset.release_id,
        asset.id,
        &file_name,
    ))
    .bind(storage_kind)
    .bind(&parts_json)
    .bind(asset.id)
    .execute(pool)
    .await?;

    println!("migrated {} {} -> {}", asset.kind, asset.id, main_url);

    if split_archive {
        println!(
            "split archive {}: {} parts migrated",
            asset.id,
            part_list.len()
        );
    } else {
        if asset.size > 0 && asset.size as u64 != main_bytes.len() as u64 {
            eprintln!(
                "warning: size changed for {}: was {}, now {}",
                asset.id,
                asset.size,
                main_bytes.len()
            );
        }
        if let Some(expected) = asset.sha256.as_deref() {
            if !expected.eq_ignore_ascii_case(&main_sha) {
                eprintln!(
                    "warning: sha256 changed for {}: was {}, now {}",
                    asset.id, expected, main_sha
                );
            }
        }
    }

    if storage.kind() == StorageKind::Local {
        println!("note: local storage used; files are in STORAGE_DIR");
    }
    Ok(())
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("download failed for {url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download failed for {url}: HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("download body failed for {url}: {error}"))?;
    Ok(bytes.to_vec())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn file_name_from_url(url: &str, fallback: &str) -> String {
    url.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn content_type(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "zip" => "application/zip",
        "json" => "application/json",
        "jar" => "application/java-archive",
        _ => "application/octet-stream",
    }
}
