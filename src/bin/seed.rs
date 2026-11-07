use std::fs;

use chrono::{NaiveDate, TimeZone, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use lavunio_api::config::Config;
use lavunio_api::db;

const CATALOG_BASE: &str =
    "https://raw.githubusercontent.com/tommy-whinston/Lavunio-Catalog/main/modes";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let pool = db::pool(&config.database_url);
    db::migrate(&pool).await;

    let catalog: Value = serde_json::from_str(&fs::read_to_string("seed/modes.json")?)?;
    let user_id = upsert_user(&pool, "tm", "TM", "admin").await?;

    let modes = catalog["modes"].as_array().cloned().unwrap_or_default();
    for mode in &modes {
        seed_mode(&pool, user_id, mode).await?;
    }

    println!("seed complete: {} modes", modes.len());
    Ok(())
}

async fn upsert_user(
    pool: &PgPool,
    username: &str,
    display_name: &str,
    role: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (id, username, display_name, role)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (username) DO UPDATE
            SET display_name = EXCLUDED.display_name, role = EXCLUDED.role
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(username)
    .bind(display_name)
    .bind(role)
    .fetch_one(pool)
    .await
}

async fn seed_mode(
    pool: &PgPool,
    user_id: Uuid,
    mode: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let slug = mode["id"].as_str().unwrap_or_default();
    let title = mode["title"].as_str().unwrap_or(slug);
    let short_description = mode["shortDescription"].as_str().unwrap_or_default();
    let description = mode["description"].as_str().unwrap_or_default();
    let cover_url = mode["imageUrl"].as_str();
    let version = mode["version"].as_str().unwrap_or("1.0.0");
    let changelog = mode["changelog"].as_str().unwrap_or_default();

    let mode_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO modes
            (id, slug, title, short_description, description, status, cover_url, author_id,
             mc_version, loader, loader_version, size_label)
        VALUES ($1, $2, $3, $4, $5, 'published', $6, $7, $8, $9, $10, $11)
        ON CONFLICT (slug) DO UPDATE
            SET title = EXCLUDED.title,
                short_description = EXCLUDED.short_description,
                description = EXCLUDED.description,
                status = 'published',
                cover_url = EXCLUDED.cover_url,
                author_id = EXCLUDED.author_id,
                mc_version = EXCLUDED.mc_version,
                loader = EXCLUDED.loader,
                loader_version = EXCLUDED.loader_version,
                size_label = EXCLUDED.size_label,
                updated_at = now()
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(slug)
    .bind(title)
    .bind(short_description)
    .bind(description)
    .bind(cover_url)
    .bind(user_id)
    .bind(mode["mcVersion"].as_str().unwrap_or_default())
    .bind(mode["loader"].as_str().unwrap_or("vanilla"))
    .bind(mode["loaderVersion"].as_str())
    .bind(mode["size"].as_str())
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO mode_authors (mode_id, user_id, role, sort_order) VALUES ($1, $2, 'author', 0) ON CONFLICT DO NOTHING",
    )
    .bind(mode_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM mode_tags WHERE mode_id = $1")
        .bind(mode_id)
        .execute(pool)
        .await?;

    for tag in mode["tags"].as_array().cloned().unwrap_or_default() {
        let Some(name) = tag.as_str() else {
            continue;
        };
        let tag_slug = lavunio_api::slugify(name);
        if tag_slug.is_empty() {
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
        .bind(&tag_slug)
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

    let published_at = mode["releaseDate"]
        .as_str()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|date| Utc.from_utc_datetime(&date));

    let release_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO releases (id, mode_id, version, status, channel, changelog, published_at)
        VALUES ($1, $2, $3, 'published', 'stable', $4, $5)
        ON CONFLICT (mode_id, version) DO UPDATE
            SET status = 'published',
                changelog = EXCLUDED.changelog,
                published_at = EXCLUDED.published_at
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(mode_id)
    .bind(version)
    .bind(changelog)
    .bind(published_at)
    .fetch_one(pool)
    .await?;

    sqlx::query("DELETE FROM release_assets WHERE release_id = $1")
        .bind(release_id)
        .execute(pool)
        .await?;

    if let Some(cover_url) = cover_url {
        sqlx::query(
            "INSERT INTO release_assets (id, release_id, kind, url, size) VALUES ($1, $2, 'cover', $3, 0)",
        )
        .bind(Uuid::new_v4())
        .bind(release_id)
        .bind(cover_url)
        .execute(pool)
        .await?;
    }

    let manifest_path = format!("seed/{slug}.manifest.json");
    if let Ok(manifest_text) = fs::read_to_string(&manifest_path) {
        let manifest: Value = serde_json::from_str(&manifest_text)?;
        if let Some(archive) = manifest.get("archive") {
            let path = archive["path"].as_str().unwrap_or("build.zip");
            let size = archive["size"].as_i64().unwrap_or(0);
            let sha256 = archive["sha256"].as_str();
            let parts = archive
                .get("parts")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new()));
            let url = format!("{CATALOG_BASE}/{slug}/{path}");

            sqlx::query(
                r#"
                INSERT INTO release_assets (id, release_id, kind, url, size, sha256, parts, manifest)
                VALUES ($1, $2, 'archive', $3, $4, $5, $6, $7)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(release_id)
            .bind(url)
            .bind(size)
            .bind(sha256)
            .bind(parts)
            .bind(&manifest)
            .execute(pool)
            .await?;
        }
    }

    println!("seeded mode {slug} ({version})");
    Ok(())
}
