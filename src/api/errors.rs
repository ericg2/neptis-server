use rocket::{http::Status, response::{status, Responder}};
use rustic_core::RusticError;
use thiserror::Error;
use rocket::serde::json::json;

#[derive(Debug, Error)]
pub enum ValidateError {
    #[error("A value is requred: {0}")]
    ValueRequired(String),

    #[error("A value is out of range: {0}")]
    OutOfRange(String),

    #[error("A value has a bad combination: {0}")]
    BadCombo(String),

    #[error("A generic error has occurred: {0}")]
    CustomError(String),
}

#[derive(Debug, Error)]
pub enum NeptisError {
    #[error(transparent)]
    DbQuery(#[from] diesel::result::Error),

    #[error(transparent)]
    DbConnection(#[from] diesel::result::ConnectionError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Internal Error: {0}")]
    InternalError(String),

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error(transparent)]
    Validation(#[from] ValidateError),

    #[error(transparent)]
    RusticJob(#[from] Box<RusticError>),

    #[error("A timeout error has occurred.")]
    Timeout,
}

impl NeptisError {
    pub fn enum_not_found(msg: String) -> Self {
        Self::InternalError(msg)
    }
}

impl<'r> Responder<'r, 'static> for NeptisError {
    fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        let status = match self {
            NeptisError::DbQuery(_) => Status::NotFound,
            NeptisError::DbConnection(_) => Status::InternalServerError,
            NeptisError::InternalError(_) => Status::InternalServerError,
            NeptisError::BadRequest(_) => Status::BadRequest,
            NeptisError::Unauthorized(_) => Status::Unauthorized,
            NeptisError::Validation(_) => Status::BadRequest,
            NeptisError::Timeout => Status::RequestTimeout,
            NeptisError::IoError(_) => Status::InternalServerError,
            NeptisError::RusticJob(_) => Status::InternalServerError,
        };
        status::Custom(status, json!({"error": self.to_string()})).respond_to(request)
    }
}
