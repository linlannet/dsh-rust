//! dsh-runtime 骨架入口。
//!
//! 当前只验证依赖装配与请求校验，不启动 HTTP 服务。服务端接入将在
//! 确定运行时与 Web 框架后加入。

use dsh_case::{RunAgent, RunRequest};
use dsh_http::routes;
use dsh_infra::InMemorySessionRepository;
use dsh_types::{AgentId, CONTRACT_VERSION};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = RunRequest::new(AgentId::parse("agent-1")?, "ping");
    request.validate()?;

    let use_case = RunAgent::new(InMemorySessionRepository::default());

    println!(
        "dsh-runtime {} contract {}",
        env!("CARGO_PKG_VERSION"),
        CONTRACT_VERSION
    );
    println!(
        "routes health={} message={} stream={}",
        routes::HEALTH,
        routes::AGENT_MESSAGE,
        routes::AGENT_STREAM
    );
    println!("session store empty: {}", use_case.repository().is_empty());

    Ok(())
}
