use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "RankingForge HTTP API server")]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "PORT", default_value = "3000")]
    pub port: u16,

    #[arg(long, env = "BIND_ADDR", default_value = "127.0.0.1")]
    pub bind_addr: String,

    /// Secret key used to sign session cookies. Must be at least 32 bytes.
    /// Override in production — the default is not secure.
    #[arg(
        long,
        env = "SESSION_SECRET",
        default_value = "change-me-in-production-not-secure-at-all"
    )]
    pub session_secret: String,

    /// start.gg API bearer token — used for the /games proxy endpoint.
    #[arg(long, env = "STARTGG_API_KEY")]
    pub startgg_api_key: String,

    /// Allowed CORS origin. Set to http://localhost:5173 for local dev.
    #[arg(
        long,
        env = "CORS_ORIGIN",
        default_value = "https://rankingforge.example.com"
    )]
    pub cors_origin: String,

    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub rust_log: String,
}
