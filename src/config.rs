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
        }
    }
}
