use std::fmt;

/// 标识符校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// 空字符串。
    Empty,
    /// 超过 128 字节。
    TooLong,
    /// 含不允许的字符。
    InvalidChar(char),
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("identifier must not be empty"),
            Self::TooLong => formatter.write_str("identifier exceeds 128 bytes"),
            Self::InvalidChar(ch) => {
                write!(formatter, "identifier contains invalid character {ch:?}")
            }
        }
    }
}

impl std::error::Error for IdError {}

/// 受限标识符：1..=128 字节，仅允许字母、数字与 `._:-`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(String);

impl Id {
    /// 按契约规则解析标识符。
    pub fn parse(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdError::Empty);
        }
        if value.len() > 128 {
            return Err(IdError::TooLong);
        }
        if let Some(ch) = value
            .chars()
            .find(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-')))
        {
            return Err(IdError::InvalidChar(ch));
        }
        Ok(Self(value))
    }

    /// 返回原始字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Agent 标识。
pub type AgentId = Id;
/// 会话标识。
pub type SessionId = Id;
/// 单次运行标识。
pub type RunId = Id;
/// 工具调用标识。
pub type ToolCallId = Id;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contract_identifier() {
        let id = Id::parse("agent-1").expect("valid identifier");
        assert_eq!(id.as_str(), "agent-1");
        assert_eq!(id.to_string(), "agent-1");
    }

    #[test]
    fn rejects_empty_identifier() {
        assert_eq!(Id::parse(""), Err(IdError::Empty));
    }

    #[test]
    fn rejects_too_long_identifier() {
        assert_eq!(Id::parse("x".repeat(129)), Err(IdError::TooLong));
    }

    #[test]
    fn rejects_invalid_character() {
        assert!(matches!(
            Id::parse("bad id"),
            Err(IdError::InvalidChar(' '))
        ));
    }
}
