use axum::http::{header, HeaderValue, Method};
use clap::Parser;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod config;
mod error;
mod routes;
mod state;

use config::Config;
use state::AppState;

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

    let http = reqwest::Client::builder()
        .user_agent("rankingforge/0.1")
        .build()
        .expect("failed to build reqwest client");

    let cors_origin: HeaderValue = config
        .cors_origin
        .parse()
        .expect("CORS_ORIGIN is not a valid header value");

    let state = AppState {
        db: pool,
        http,
        session_secret: config.session_secret,
        cors_origin: config.cors_origin,
    };

    let cors = CorsLayer::new()
        .allow_origin(cors_origin)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE]);

    let app = routes::router()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", config.bind_addr, config.port)
        .parse()
        .expect("invalid bind address");

    tracing::info!("API listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}
