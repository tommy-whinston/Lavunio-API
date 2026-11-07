use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub fn pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy(database_url)
        .expect("invalid DATABASE_URL")
}

pub async fn migrate(pool: &PgPool) {
    let migration = tokio::time::timeout(
        Duration::from_secs(8),
        sqlx::migrate!("./migrations").run(pool),
    )
    .await;

    match migration {
        Ok(Ok(())) => tracing::info!("database migrations applied"),
        Ok(Err(error)) => tracing::warn!("database migrations failed: {error}"),
        Err(_) => tracing::warn!("database is not available; migrations skipped"),
    }
}
