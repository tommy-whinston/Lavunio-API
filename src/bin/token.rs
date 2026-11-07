use tmstore_api::auth;
use tmstore_api::config::Config;
use tmstore_api::db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let pool = db::pool(&config.database_url);
    db::migrate(&pool).await;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin token -- <username> <token-name> [scopes]");
        eprintln!("Example: cargo run --bin token -- tommy \"PC publisher\" publisher");
        std::process::exit(1);
    }

    let username = args[0].trim();
    let name = args[1].trim();
    let scopes: Vec<String> = args
        .get(2)
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec!["publisher".to_string()]);

    let token = auth::create_token_for_user(&pool, username, name, &scopes).await?;
    println!("Token for {username}:");
    println!("{token}");
    println!();
    println!("Сохрани токен: он больше не будет показан.");
    Ok(())
}
