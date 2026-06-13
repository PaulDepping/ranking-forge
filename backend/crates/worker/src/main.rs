use clap::Parser;
use sqlx::postgres::PgListener;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tokio::task::JoinHandle;
use uuid::Uuid;

mod compute;
mod config;
mod import;
use config::Config;

fn init_tracing(rust_log: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(rust_log))
        .init();
}

async fn shutdown(pool: &sqlx::PgPool, in_flight: Vec<(Uuid, JoinHandle<()>)>) {
    let job_ids: Vec<Uuid> = in_flight.iter().map(|(id, _)| *id).collect();
    for (_, handle) in &in_flight {
        handle.abort();
    }
    if job_ids.is_empty() {
        tracing::info!("shutdown: no in-flight jobs");
        return;
    }
    tracing::info!(
        count = job_ids.len(),
        "shutdown: aborting in-flight imports"
    );
    if let Err(e) = common::jobs::mark_shutdown(pool, &job_ids).await {
        tracing::error!(%e, "shutdown: failed to mark in-flight jobs as failed");
    }
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

    let mut listener = PgListener::connect(&config.database_url)
        .await
        .expect("failed to create PgListener");
    listener
        .listen("jobs")
        .await
        .expect("failed to listen on jobs channel");

    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");

    tracing::info!("Worker ready, listening for import jobs");

    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));
    cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut in_flight: Vec<(Uuid, JoinHandle<()>)> = Vec::new();

    loop {
        // Reap handles for tasks that finished since the last iteration
        in_flight.retain(|(_, h)| !h.is_finished());

        // Drain all pending jobs before waiting
        loop {
            match common::jobs::claim(&pool).await {
                Ok(Some(job)) => {
                    let pool2 = pool.clone();
                    let project_id = job.project_id;
                    let job_id = job.id;

                    let handle = match job.kind.as_str() {
                        "import_tournaments" => {
                            let import_params = common::jobs::ImportParams::from_job(&job);
                            let api_key = match sqlx::query_scalar!(
                                "SELECT u.startgg_api_key FROM projects rp
                                 JOIN users u ON u.id = rp.owner_id
                                 WHERE rp.id = $1",
                                project_id,
                            )
                            .fetch_optional(&pool)
                            .await
                            {
                                Ok(Some(Some(key))) => key,
                                Ok(_) => {
                                    tracing::error!(%job_id, %project_id, "project owner has no start.gg API key");
                                    let _ = common::jobs::mark_failed(
                                        &pool,
                                        job_id,
                                        "Project owner has no start.gg API key configured",
                                    )
                                    .await;
                                    continue;
                                }
                                Err(e) => {
                                    tracing::error!(%e, %job_id, "failed to look up owner API key");
                                    let _ = common::jobs::mark_failed(&pool, job_id, &e.to_string()).await;
                                    continue;
                                }
                            };
                            let startgg = common::startgg::StartggClient::new(api_key);
                            tracing::info!(%job_id, %project_id, "starting import");
                            tokio::spawn(async move {
                                match import::run(&pool2, &startgg, project_id, job_id, import_params).await {
                                    Ok(()) => {
                                        tracing::info!(%job_id, "import complete");
                                        if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                                            tracing::error!(%e, %job_id, "failed to mark job done");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(%e, %job_id, "import failed");
                                        if let Err(e2) = common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await {
                                            tracing::error!(%e2, %job_id, "failed to mark job failed");
                                        }
                                    }
                                }
                            })
                        }
                        "compute_ranking" => {
                            let params = common::jobs::ComputeRankingParams::from_job(&job);
                            tracing::info!(%job_id, ranking_id = %params.ranking_id, "starting compute_ranking");
                            tokio::spawn(async move {
                                match compute::run(&pool2, params.ranking_id).await {
                                    Ok(()) => {
                                        tracing::info!(%job_id, "compute_ranking complete");
                                        if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                                            tracing::error!(%e, %job_id, "failed to mark job done");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(%e, %job_id, "compute_ranking failed");
                                        if let Err(e2) = common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await {
                                            tracing::error!(%e2, %job_id, "failed to mark job failed");
                                        }
                                    }
                                }
                            })
                        }
                        kind => {
                            tracing::warn!(%job_id, %kind, "unknown job kind, marking failed");
                            let _ = common::jobs::mark_failed(&pool, job_id, &format!("unknown job kind: {kind}")).await;
                            continue;
                        }
                    };
                    in_flight.push((job_id, handle));
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::error!(%e, "error claiming job");
                    break;
                }
            }
        }

        // Wait for a NOTIFY, poll every 30s, or shutdown signal
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
            _ = cleanup_interval.tick() => {
                if let Err(e) = sqlx::query!("DELETE FROM sessions WHERE expires_at < NOW()")
                    .execute(&pool)
                    .await
                {
                    tracing::error!(%e, "failed to clean up expired sessions");
                }
            }
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, shutting down");
                shutdown(&pool, in_flight).await;
                return;
            }
            _ = sigint.recv() => {
                tracing::info!("received SIGINT, shutting down");
                shutdown(&pool, in_flight).await;
                return;
            }
        }
    }
}
