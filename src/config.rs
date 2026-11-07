use std::env;
use std::path::PathBuf;

use crate::storage::StorageKind;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub public_base_url: String,
    pub storage_kind: StorageKind,
    pub storage_dir: PathBuf,
    pub s3_endpoint: Option<String>,
    pub s3_region: String,
    pub s3_bucket: Option<String>,
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_public_base_url: Option<String>,
    pub s3_force_path_style: bool,
    pub s3_presign_expires_secs: u64,
    pub bootstrap_token: Option<String>,
    pub dev_mode: bool,
    pub access_token_ttl_secs: u64,
    pub refresh_token_ttl_secs: u64,
    pub email_token_ttl_secs: u64,
    pub reset_token_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://lavunio:lavunio@localhost:5432/lavunio".to_string());
        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8080);
        let public_base_url =
            env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| format!("http://{host}:{port}"));
        let storage_kind = match env::var("STORAGE_KIND").as_deref() {
            Ok("s3") => StorageKind::S3,
            Ok("local") | Err(_) => StorageKind::Local,
            Ok(other) => {
                tracing::warn!("unknown STORAGE_KIND={other}, using local storage");
                StorageKind::Local
            }
        };
        let storage_dir = env::var("STORAGE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data/uploads"));
        let s3_endpoint = env::var("S3_ENDPOINT")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_region = env::var("S3_REGION").unwrap_or_else(|_| "auto".to_string());
        let s3_bucket = env::var("S3_BUCKET")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_access_key_id = env::var("S3_ACCESS_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_secret_access_key = env::var("S3_SECRET_ACCESS_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_public_base_url = env::var("S3_PUBLIC_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let s3_force_path_style = env::var("S3_FORCE_PATH_STYLE")
            .map(|value| value != "false" && value != "0")
            .unwrap_or(true);
        let s3_presign_expires_secs = env::var("S3_PRESIGN_EXPIRES_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(3600);
        let bootstrap_token = env::var("LAVUNIO_BOOTSTRAP_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let dev_mode = env_bool("LAVUNIO_DEV_MODE", false);
        let access_token_ttl_secs = env_u64("LAVUNIO_ACCESS_TTL_SECS", 15 * 60);
        let refresh_token_ttl_secs = env_u64("LAVUNIO_REFRESH_TTL_SECS", 30 * 24 * 60 * 60);
        let email_token_ttl_secs = env_u64("LAVUNIO_EMAIL_TTL_SECS", 24 * 60 * 60);
        let reset_token_ttl_secs = env_u64("LAVUNIO_RESET_TTL_SECS", 60 * 60);

        Self {
            database_url,
            host,
            port,
            public_base_url,
            storage_kind,
            storage_dir,
            s3_endpoint,
            s3_region,
            s3_bucket,
            s3_access_key_id,
            s3_secret_access_key,
            s3_public_base_url,
            s3_force_path_style,
            s3_presign_expires_secs,
            bootstrap_token,
            dev_mode,
            access_token_ttl_secs,
            refresh_token_ttl_secs,
            email_token_ttl_secs,
            reset_token_ttl_secs,
        }
    }

    pub fn local(database_url: String, storage_dir: PathBuf) -> Self {
        Self {
            database_url,
            host: "127.0.0.1".to_string(),
            port: 0,
            public_base_url: "http://test".to_string(),
            storage_kind: StorageKind::Local,
            storage_dir,
            s3_endpoint: None,
            s3_region: "auto".to_string(),
            s3_bucket: None,
            s3_access_key_id: None,
            s3_secret_access_key: None,
            s3_public_base_url: None,
            s3_force_path_style: true,
            s3_presign_expires_secs: 3600,
            bootstrap_token: None,
            dev_mode: true,
            access_token_ttl_secs: 15 * 60,
            refresh_token_ttl_secs: 30 * 24 * 60 * 60,
            email_token_ttl_secs: 24 * 60 * 60,
            reset_token_ttl_secs: 60 * 60,
        }
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            let value = value.trim().to_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}
