use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::startgg::StartggError;

#[derive(Debug)]
pub enum AppError {
    NotFound,
    Unauthorized,
    UnprocessableEntity(String),
    Db(sqlx::Error),
    PasswordHash,
    ExternalApi(reqwest::Error),
    ExternalApiError,
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::ExternalApi(e)
    }
}

impl From<StartggError> for AppError {
    fn from(e: StartggError) -> Self {
        match e {
            StartggError::Http(re) => AppError::ExternalApi(re),
            StartggError::GraphQL(msg) => {
                tracing::error!("start.gg GraphQL error: {msg}");
                AppError::ExternalApiError
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg): (StatusCode, String) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".into()),
            AppError::UnprocessableEntity(m) => (StatusCode::UNPROCESSABLE_ENTITY, m.clone()),
            AppError::Db(sqlx::Error::RowNotFound) => (StatusCode::NOT_FOUND, "not found".into()),
            AppError::Db(e) => {
                tracing::error!("database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".into(),
                )
            }
            AppError::PasswordHash => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".into(),
            ),
            AppError::ExternalApi(e) => {
                tracing::error!("external API error: {e}");
                (StatusCode::BAD_GATEWAY, "upstream API error".into())
            }
            AppError::ExternalApiError => (StatusCode::BAD_GATEWAY, "upstream API error".into()),
        };
        (status, Json(serde_json::json!({ "message": msg }))).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
