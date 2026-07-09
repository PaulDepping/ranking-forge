use chrono::NaiveDate;
use clap::Parser;

fn default_from_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2015, 1, 1).unwrap()
}

fn default_to_date() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

#[derive(Debug, Parser)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "STARTGG_API_KEY")]
    pub startgg_api_key: String,

    #[arg(long, env = "CRAWLER_FROM_DATE", default_value_t = default_from_date())]
    pub from_date: NaiveDate,

    #[arg(long, env = "CRAWLER_TO_DATE", default_value_t = default_to_date())]
    pub to_date: NaiveDate,

    #[arg(long, env = "CRAWLER_WINDOW_DAYS", default_value_t = 7)]
    pub window_days: u32,

    #[arg(long, env = "CRAWLER_DELAY_MS", default_value_t = 750)]
    pub delay_ms: u64,

    #[arg(long, env = "CRAWLER_SETS_PER_PAGE", default_value_t = 20)]
    pub sets_per_page: u32,

    #[arg(long, env = "CRAWLER_GAME_ID")]
    pub game_id: Option<u64>,

    #[arg(long, env = "RUST_LOG", default_value_t = String::from("info"))]
    pub rust_log: String,

    #[arg(long, env = "STARTGG_BASE_URL", default_value_t = crate::api::STARTGG_API_URL.to_string())]
    pub startgg_base_url: String,
}
