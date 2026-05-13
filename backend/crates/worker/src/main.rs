use clap::Parser;
use sqlx::postgres::PgListener;
use std::time::Duration;

mod config;
mod import;
use config::Config;

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

    let startgg = common::startgg::StartggClient::new(config.startgg_api_key.into());

    let mut listener = PgListener::connect(&config.database_url)
        .await
        .expect("failed to create PgListener");
    listener
        .listen("jobs")
        .await
        .expect("failed to listen on jobs channel");

    tracing::info!("Worker ready, listening for import jobs");

    loop {
        // Drain all pending jobs before waiting
        loop {
            match common::jobs::claim(&pool).await {
                Ok(Some(job)) => {
                    let pool2 = pool.clone();
                    let startgg2 = startgg.clone();
                    let project_id = job.project_id;
                    let job_id = job.id;
                    tracing::info!(%job_id, %project_id, "starting import");
                    tokio::spawn(async move {
                        match import::run(&pool2, &startgg2, project_id).await {
                            Ok(()) => {
                                tracing::info!(%job_id, "import complete");
                                if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                                    tracing::error!(%e, %job_id, "failed to mark job done");
                                }
                            }
                            Err(e) => {
                                tracing::error!(%e, %job_id, "import failed");
                                if let Err(e2) =
                                    common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await
                                {
                                    tracing::error!(%e2, %job_id, "failed to mark job failed");
                                }
                            }
                        }
                    });
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::error!(%e, "error claiming job");
                    break;
                }
            }
        }

        // Wait for a NOTIFY or poll every 30s
        tokio::select! {
            result = listener.recv() => {
                match result {
                    Ok(_) => tracing::debug!("received job notification"),
                    Err(e) => tracing::error!(%e, "PgListener error"),
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                tracing::debug!("polling for jobs");
            }
        }
    }
}
