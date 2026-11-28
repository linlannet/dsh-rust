use std::fmt;

use serde::{Deserialize, Serialize};

/// 稳定错误码。字符串形式与契约中的 `code` 对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// 请求非法。
    InvalidRequest,
    /// 资源不存在。
    NotFound,
    /// 状态冲突。
    Conflict,
    /// 超时。
    Timeout,
    /// 已取消。
    Cancelled,
    /// 内部错误。
    Internal,
    /// 依赖暂不可用。
    Unavailable,
    /// 存储暂不可用。
    StorageUnavailable,
    /// 上游响应非法。
    InvalidResponse,
    /// 单次运行的事件输出超出字节预算。
    StreamBudgetExceeded,
}

impl ErrorCode {
    /// 返回稳定的机器可读名称。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
            Self::Unavailable => "unavailable",
            Self::StorageUnavailable => "storage_unavailable",
            Self::InvalidResponse => "invalid_response",
            Self::StreamBudgetExceeded => "stream_budget_exceeded",
        }
    }
}

/// 运行时错误。消息文本不承诺稳定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    /// 稳定错误码。
    code: ErrorCode,
    /// 可读消息，不保证稳定。
    message: String,
    /// 是否可重试。
    #[serde(default, skip_serializing_if = "is_false")]
    retryable: bool,
}

const fn is_false(value: &bool) -> bool {
    !*value
}

impl Error {
    /// 构造错误。
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    /// 标记为可重试。
    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    /// 返回错误码。
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// 返回错误消息。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 是否可重试。
    pub const fn is_retryable(&self) -> bool {
        self.retryable
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_builder_sets_flag() {
        let error = Error::new(ErrorCode::Timeout, "timed out").retryable();
        assert_eq!(error.code(), ErrorCode::Timeout);
        assert_eq!(error.message(), "timed out");
        assert!(error.is_retryable());
        assert_eq!(error.to_string(), "timeout: timed out");
    }

    #[test]
    fn serializes_as_contract_error() {
        let error = Error::new(ErrorCode::InvalidRequest, "bad input");
        let json = serde_json::to_value(&error).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({"code": "invalid_request", "message": "bad input"})
        );
    }

    #[test]
    fn error_codes_use_contract_strings() {
        let cases = [
            (ErrorCode::InvalidRequest, "invalid_request"),
            (ErrorCode::NotFound, "not_found"),
            (ErrorCode::Conflict, "conflict"),
            (ErrorCode::Timeout, "timeout"),
            (ErrorCode::Cancelled, "cancelled"),
            (ErrorCode::Internal, "internal"),
            (ErrorCode::Unavailable, "unavailable"),
            (ErrorCode::StorageUnavailable, "storage_unavailable"),
            (ErrorCode::InvalidResponse, "invalid_response"),
            (ErrorCode::StreamBudgetExceeded, "stream_budget_exceeded"),
        ];
        for (code, text) in cases {
            assert_eq!(code.as_str(), text);
        }
    }
}
