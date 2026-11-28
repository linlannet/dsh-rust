//! HTTP 错误映射。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dsh_types::{Error, ErrorCode};

use crate::dto::ErrorBody;

/// 带状态码与契约错误体的 HTTP 错误。
#[derive(Debug, Clone)]
pub struct HttpError {
    /// HTTP 状态码。
    pub status: StatusCode,
    /// 错误体。
    pub body: ErrorBody,
}

impl HttpError {
    /// 附加 traceId。
    pub fn with_trace(mut self, trace_id: Option<String>) -> Self {
        self.body.trace_id = trace_id;
        self
    }
}

impl From<Error> for HttpError {
    fn from(error: Error) -> Self {
        let status = match error.code() {
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::Timeout => StatusCode::GATEWAY_TIMEOUT,
            ErrorCode::Cancelled => StatusCode::CONFLICT,
            ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::InvalidResponse => StatusCode::BAD_GATEWAY,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            body: ErrorBody {
                code: error.code().as_str(),
                message: error.message().to_owned(),
                retryable: error.is_retryable().then_some(true),
                trace_id: None,
            },
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
