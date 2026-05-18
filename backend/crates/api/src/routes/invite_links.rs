use axum::{Router, http::StatusCode, routing::get};
use crate::state::AppState;
pub fn router() -> Router<AppState> {
    Router::new().route("/", get(|| async { StatusCode::NOT_IMPLEMENTED }))
}
