use buddy_database::DbError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CalendarError {
    #[error("{0}")]
    Message(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("scheduling conflict")]
    Conflict(String),
}

impl From<DbError> for CalendarError {
    fn from(value: DbError) -> Self {
        match value {
            DbError::NotFound(id) => Self::NotFound(id),
            other => Self::Database(other.to_string()),
        }
    }
}

impl CalendarError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::InvalidInput(_) => "invalid_input",
            Self::Database(_) => "database",
            Self::Message(_) => "error",
            Self::Conflict(_) => "conflict",
        }
    }
}
