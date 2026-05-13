use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project,
    state::AppState,
};
use common::models::Job;

#[derive(Serialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        JobResponse {
            id: j.id,
            status: j.status,
            error: j.error,
            created_at: j.created_at,
            updated_at: j.updated_at,
        }
    }
}

pub async fn start_import(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;
    let job = common::jobs::enqueue(&state.db, project_id).await?;
    tracing::info!(user_id = %user.id, %project_id, job_id = %job.id, "import job enqueued");
    Ok((StatusCode::ACCEPTED, Json(JobResponse::from(job))))
}

pub async fn get_import_status(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;
    let job = common::jobs::latest_for_project(&state.db, project_id)
        .await?
        .ok_or(AppError::NotFound)?;
    tracing::debug!(user_id = %user.id, %project_id, job_id = %job.id, status = %job.status, "import status queried");
    Ok(Json(JobResponse::from(job)))
}
