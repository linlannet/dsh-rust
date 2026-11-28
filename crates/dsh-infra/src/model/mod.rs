//! 模型适配器。

pub mod fake;

#[cfg(feature = "deepseek")]
pub mod deepseek;

pub use fake::{FakeModel, FakeScript};

#[cfg(feature = "deepseek")]
pub use deepseek::DeepSeekModel;
