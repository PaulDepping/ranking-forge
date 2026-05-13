use axum::http::{HeaderName, HeaderValue, Method, Request, header};
use clap::Parser;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{MakeSpan, TraceLayer};

use api::config::Config;
use api::state::AppState;
use common::startgg::StartggClient;

static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Default)]
struct SpanWithRequestId;

impl<B> MakeSpan<B> for SpanWithRequestId {
    fn make_span(&mut self, request: &Request<B>) -> tracing::Span {
        let request_id = request
            .headers()
            .get(&X_REQUEST_ID)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        tracing::info_span!(
            "request",
            method = %request.method(),
            uri = %request.uri().path(),
            request_id,
        )
    }
}

fn init_tracing(rust_log: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(rust_log))
        .init();
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = Config::parse();

    init_tracing(&config.rust_log);

    let pool = common::db::connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    let cors_origin: HeaderValue = config
        .cors_origin
        .parse()
        .expect("CORS_ORIGIN is not a valid header value");

    let state = AppState {
        db: pool,
        startgg: StartggClient::new(config.startgg_api_key),
        session_secret: config.session_secret,
        cors_origin: config.cors_origin,
    };

    let cors = CorsLayer::new()
        .allow_origin(cors_origin)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE]);

    let app = api::routes::router()
        .layer(TraceLayer::new_for_http().make_span_with(SpanWithRequestId::default()))
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID.clone()))
        .layer(SetRequestIdLayer::new(X_REQUEST_ID.clone(), MakeRequestUuid))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::new(config.bind_addr, config.port);

    tracing::info!("API listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}
