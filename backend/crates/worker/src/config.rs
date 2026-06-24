use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "RankingForge background import worker")]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "RUST_LOG", default_value_t = String::from("info"))]
    pub rust_log: String,
}
