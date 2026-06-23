use clap::Parser;
use cli::Config;

mod api;
mod api_types;
mod cli;
mod db;
mod scraper;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let _config = Config::parse();
}
