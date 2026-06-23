use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GqlResponse<T> {
    pub errors: Option<Vec<GqlError>>,
    pub success: Option<bool>,
    pub data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GqlError {
    pub message: Option<String>,
    pub extensions: Option<GqlErrorExtensions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GqlErrorExtensions {
    pub category: Option<String>,
}
