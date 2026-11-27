//! 领域模型与端口。
//!
//! 只依赖 `dsh-types`，不依赖 Web 框架、数据库驱动或具体模型 SDK。

pub mod ports;
pub mod session;

pub use ports::SessionRepository;
pub use session::Session;
