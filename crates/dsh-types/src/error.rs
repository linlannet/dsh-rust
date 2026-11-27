use std::fmt;

/// 稳定错误码。字符串形式与契约中的 `code` 对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        }
    }
}

/// 运行时错误。消息文本不承诺稳定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    code: ErrorCode,
    message: String,
    retryable: bool,
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
}
