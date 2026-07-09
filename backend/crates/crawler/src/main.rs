use clap::Parser;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal::unix::{SignalKind, signal};

use crawler::{cli, scraper};

fn init_tracing(rust_log: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(rust_log))
        .init();
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = cli::Config::parse();

    init_tracing(&config.rust_log);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");

    tokio::select! {
        result = scraper::run(&config, &pool, &shutdown) => {
            if let Err(e) = result {
                tracing::error!(%e, "crawler exited with error");
                std::process::exit(1);
            }
        }
        _ = sigterm.recv() => {
            tracing::info!("received SIGTERM, shutting down");
            shutdown_clone.store(true, Ordering::SeqCst);
        }
        _ = sigint.recv() => {
            tracing::info!("received SIGINT, shutting down");
            shutdown_clone.store(true, Ordering::SeqCst);
        }
    }
}
