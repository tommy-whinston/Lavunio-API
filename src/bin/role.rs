use lavunio_api::config::Config;
use lavunio_api::db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let pool = db::pool(&config.database_url);
    db::migrate(&pool).await;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: cargo run --bin role -- <email|username> <player|author|moderator|admin>"
        );
        eprintln!("Example: cargo run --bin role -- author@example.com author");
        std::process::exit(1);
    }

    let role = args[1].trim().to_lowercase();
    if !matches!(role.as_str(), "player" | "author" | "moderator" | "admin") {
        eprintln!("Неизвестная роль: {role}");
        std::process::exit(1);
    }

    let updated = sqlx::query(
        r#"
        UPDATE users
        SET role = $2, updated_at = now()
        WHERE lower(email) = lower($1) OR lower(username) = lower($1)
        "#,
    )
    .bind(args[0].trim())
    .bind(&role)
    .execute(&pool)
    .await?;

    if updated.rows_affected() == 0 {
        eprintln!("Пользователь не найден: {}", args[0]);
        std::process::exit(1);
    }

    println!("Роль обновлена: {} → {role}", args[0]);
    Ok(())
}
