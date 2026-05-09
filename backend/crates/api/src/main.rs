use clap::Parser;

mod config;
use config::Config;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = Config::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(&config.rust_log))
        .init();

    let pool = common::db::connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    tracing::info!("API starting on {}:{}", config.bind_addr, config.port);
    // Router setup will go here
}
