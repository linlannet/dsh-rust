/// 消息内容块。v1 草案仅包含文本块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentBlock {
    /// 文本内容。
    Text { text: String },
}

impl ContentBlock {
    /// 构造文本块。
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// 返回文本内容。
    pub fn as_text(&self) -> &str {
        match self {
            Self::Text { text } => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_text_block() {
        let block = ContentBlock::text("hello");
        assert_eq!(block.as_text(), "hello");
    }
}
