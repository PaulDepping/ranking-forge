use std::net::{IpAddr, Ipv4Addr};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "RankingForge HTTP API server")]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "PORT", default_value_t = 3000)]
    pub port: u16,

    #[arg(long, env = "BIND_ADDR", default_value_t = Ipv4Addr::UNSPECIFIED.into())]
    pub bind_addr: IpAddr,

    /// Allowed CORS origin. Set to http://localhost:5173 for local dev.
    #[arg(long, env = "CORS_ORIGIN")]
    pub cors_origin: String,

    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub rust_log: String,
}
