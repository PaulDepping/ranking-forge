use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    extractors::ClientIpExtractor,
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::{require_project_access, require_project_read_access},
    state::AppState,
};
use common::{
    jobs::ImportParams,
    models::{Job, UserRole},
};

#[derive(Serialize, Deserialize)]
pub struct ImportProgress {
    pub phase: String,
    pub step: u32,
    pub total: u32,
}

#[derive(Serialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub after_date: Option<NaiveDate>,
    pub before_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub progress: Option<ImportProgress>,
}

impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        let params = ImportParams::from_job(&j);
        let to_date = |ts: i64| Utc.timestamp_opt(ts, 0).single().map(|dt| dt.date_naive());
        JobResponse {
            id: j.id,
            status: j.status,
            error: j.error,
            after_date: params.after_date.and_then(to_date),
            before_date: params.before_date.and_then(to_date),
            created_at: j.created_at,
            updated_at: j.updated_at,
            progress: j.progress.and_then(|v| serde_json::from_value(v).ok()),
        }
    }
}

#[derive(Deserialize, Default)]
pub struct ImportRequest {
    pub after_date: Option<NaiveDate>,
    pub before_date: Option<NaiveDate>,
}

fn date_to_timestamp(date: NaiveDate) -> i64 {
    Utc.from_utc_datetime(&date.and_time(NaiveTime::MIN))
        .timestamp()
}

pub async fn start_import(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    body: Option<Json<ImportRequest>>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let owner_key: Option<String> = sqlx::query_scalar!(
        "SELECT u.startgg_api_key FROM projects rp
         JOIN users u ON u.id = rp.owner_id
         WHERE rp.id = $1",
        project_id,
    )
    .fetch_optional(&state.db)
    .await?
    .flatten();

    if owner_key.is_none() {
        return Err(AppError::UnprocessableEntity(
            "Project owner has not configured a start.gg API key".into(),
        ));
    }

    let req = body.map(|b| b.0).unwrap_or_default();
    let params = ImportParams {
        after_date: req.after_date.map(date_to_timestamp),
        before_date: req.before_date.map(date_to_timestamp),
    };
    let job = common::jobs::enqueue(&state.db, project_id, params).await?;
    tracing::info!(user_id = %user.id, %project_id, job_id = %job.id, "import job enqueued");
    Ok((StatusCode::ACCEPTED, Json(JobResponse::from(job))))
}

pub async fn get_import_status(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    let job = common::jobs::latest_for_project(&state.db, project_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(JobResponse::from(job)))
}

pub fn rate_limited_post_router() -> Router<AppState> {
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ClientIpExtractor)
            .per_second(20)
            .burst_size(3)
            .finish()
            .expect("invalid rate-limit config"),
    );
    Router::new()
        .route("/{id}/import", post(start_import))
        .layer(GovernorLayer::new(governor_conf))
}

#[cfg(test)]
mod tests {
    use crate::{routes, state::AppState};
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    fn make_app(pool: PgPool) -> Router {
        let state = AppState {
            db: pool,
            cors_origin: "http://localhost".into(),
        };
        routes::router().with_state(state)
    }

    async fn register(app: &Router, name: &str) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "email": format!("{name}@test.com"),
                            "display_name": name,
                            "password": "password123"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        format!("session_id={}", body["session_id"].as_str().unwrap())
    }

    async fn create_project(app: &Router, cookie: &str) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": "Test Project"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_import_post_is_rate_limited(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "rl_import").await;

        sqlx::query!(
            "UPDATE users SET startgg_api_key = 'test-key' WHERE email = 'rl_import@test.com'"
        )
        .execute(&pool)
        .await
        .unwrap();

        let project_id = create_project(&app, &cookie).await;

        // First 3 requests consume the burst — must NOT be 429
        for i in 0..3 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/projects/{project_id}/import"))
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_ne!(
                resp.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "request {i} should not be rate-limited"
            );
        }

        // 4th request should be rate-limited
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/projects/{project_id}/import"))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
