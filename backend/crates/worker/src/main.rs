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

    let _pool = common::db::connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    tracing::info!("Worker starting");
    // Job loop will go here
}
