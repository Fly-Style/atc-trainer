use atc_shared::protocol::{ApiError, ApiErrorBody};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("session not found")]
    SessionNotFound,
    #[error("invalid session hash")]
    InvalidSessionHash,
    #[error("student position missing")]
    StudentPositionMissing,
    #[error("student position occupied")]
    StudentPositionOccupied,
    #[error("student position already exists")]
    StudentPositionAlreadyExists,
    #[error("session already ended")]
    SessionEnded,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::Unauthorized => "unauthorized",
            AppError::Forbidden => "forbidden",
            AppError::SessionNotFound => "session_not_found",
            AppError::InvalidSessionHash => "invalid_session_hash",
            AppError::StudentPositionMissing => "student_position_missing",
            AppError::StudentPositionOccupied => "student_position_occupied",
            AppError::StudentPositionAlreadyExists => "student_position_already_exists",
            AppError::SessionEnded => "session_ended",
            AppError::BadRequest(_) => "bad_request",
            AppError::Internal(_) => "internal_error",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::SessionNotFound => StatusCode::NOT_FOUND,
            AppError::InvalidSessionHash => StatusCode::UNAUTHORIZED,
            AppError::StudentPositionMissing => StatusCode::BAD_REQUEST,
            AppError::StudentPositionOccupied => StatusCode::CONFLICT,
            AppError::StudentPositionAlreadyExists => StatusCode::CONFLICT,
            AppError::SessionEnded => StatusCode::CONFLICT,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiError {
            error: ApiErrorBody {
                code: self.code().to_string(),
                message: self.to_string(),
            },
        };
        (self.status(), Json(body)).into_response()
    }
}
