//! 基础设施适配器。
//!
//! 骨架阶段只提供内存会话仓库，让用例与测试先跑通。数据库、模型、
//! 进程与 MCP 适配器后续按端口逐个加入。

pub mod memory;

pub use memory::InMemorySessionRepository;
