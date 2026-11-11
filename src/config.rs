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
    pub resend_api_key: Option<String>,
    pub mail_from: String,
    pub mail_reply_to: Option<String>,
    pub access_token_ttl_secs: u64,
    pub refresh_token_ttl_secs: u64,
    pub email_token_ttl_secs: u64,
    pub reset_token_ttl_secs: u64,
    pub game_token_ttl_secs: u64,
    pub skin_domains: Vec<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_window_secs: u64,
    pub rate_limit_login: u32,
    pub rate_limit_register: u32,
    pub rate_limit_reset: u32,
    pub rate_limit_verify: u32,
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
        let resend_api_key = env::var("RESEND_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mail_from = env::var("MAIL_FROM")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Lavunio <no-reply@lavunio.pro>".to_string());
        let mail_reply_to = env::var("MAIL_REPLY_TO")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let access_token_ttl_secs = env_u64("LAVUNIO_ACCESS_TTL_SECS", 15 * 60);
        let refresh_token_ttl_secs = env_u64("LAVUNIO_REFRESH_TTL_SECS", 30 * 24 * 60 * 60);
        let email_token_ttl_secs = env_u64("LAVUNIO_EMAIL_TTL_SECS", 15 * 60);
        let reset_token_ttl_secs = env_u64("LAVUNIO_RESET_TTL_SECS", 60 * 60);
        let game_token_ttl_secs = env_u64("LAVUNIO_GAME_TTL_SECS", 30 * 24 * 60 * 60);
        let skin_domains =
            env_list("SKIN_DOMAINS").unwrap_or_else(|| default_skin_domains(&public_base_url));
        let rate_limit_enabled = env_bool("LAVUNIO_RATE_LIMIT_ENABLED", true);
        let rate_limit_window_secs = env_u64("LAVUNIO_RATE_LIMIT_WINDOW_SECS", 15 * 60);
        let rate_limit_login = env_u32("LAVUNIO_RATE_LIMIT_LOGIN", 10);
        let rate_limit_register = env_u32("LAVUNIO_RATE_LIMIT_REGISTER", 10);
        let rate_limit_reset = env_u32("LAVUNIO_RATE_LIMIT_RESET", 5);
        let rate_limit_verify = env_u32("LAVUNIO_RATE_LIMIT_VERIFY", 10);

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
            resend_api_key,
            mail_from,
            mail_reply_to,
            access_token_ttl_secs,
            refresh_token_ttl_secs,
            email_token_ttl_secs,
            reset_token_ttl_secs,
            game_token_ttl_secs,
            skin_domains,
            rate_limit_enabled,
            rate_limit_window_secs,
            rate_limit_login,
            rate_limit_register,
            rate_limit_reset,
            rate_limit_verify,
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
            resend_api_key: None,
            mail_from: "Lavunio <no-reply@localhost>".to_string(),
            mail_reply_to: None,
            access_token_ttl_secs: 15 * 60,
            refresh_token_ttl_secs: 30 * 24 * 60 * 60,
            email_token_ttl_secs: 24 * 60 * 60,
            reset_token_ttl_secs: 60 * 60,
            game_token_ttl_secs: 30 * 24 * 60 * 60,
            skin_domains: vec!["127.0.0.1".to_string(), "localhost".to_string()],
            rate_limit_enabled: false,
            rate_limit_window_secs: 15 * 60,
            rate_limit_login: 1000,
            rate_limit_register: 1000,
            rate_limit_reset: 1000,
            rate_limit_verify: 1000,
        }
    }
}

fn env_list(name: &str) -> Option<Vec<String>> {
    env::var(name).ok().map(|value| {
        value
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
    })
}

fn default_skin_domains(public_base_url: &str) -> Vec<String> {
    let mut domains = vec!["127.0.0.1".to_string(), "localhost".to_string()];
    let host = public_base_url
        .split("://")
        .nth(1)
        .unwrap_or(public_base_url)
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim();
    if !host.is_empty() && !domains.iter().any(|item| item == host) {
        domains.push(host.to_string());
    }
    domains
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

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default)
}
