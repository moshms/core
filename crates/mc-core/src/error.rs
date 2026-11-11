use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl CoreError {
    #[must_use]
    pub fn not_found(entity: &'static str, id: impl Display) -> Self {
        Self::NotFound {
            entity,
            id: id.to_string(),
        }
    }
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
