use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::{require_project_access, require_project_read_access},
    state::AppState,
};
use common::{jobs::ImportParams, models::{Job, ProjectMemberRole}};

#[derive(Serialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub after_date: Option<NaiveDate>,
    pub before_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Editor).await?;
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
