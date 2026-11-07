use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub public_base_url: String,
    pub storage_dir: PathBuf,
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
        let storage_dir = env::var("STORAGE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data/uploads"));
        let bootstrap_token = env::var("LAVUNIO_BOOTSTRAP_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self {
            database_url,
            host,
            port,
            public_base_url,
            storage_dir,
            bootstrap_token,
        }
    }
}
