//! ReAct 最小循环与运行编排。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dsh_domain::{
    ChatMessage, FinishReason, ModelPort, ModelRequest, ModelStep, Role, RunEvent, RunStatus,
    Session, SessionRepository, Tool, ToolCallStatus, Usage,
};
use dsh_types::{AgentId, ContentBlock, Error, ErrorCode, JsonValue, RunId, SessionId};

use crate::event_sink::EventSink;
use crate::run_registry::RunRegistry;
use crate::tool_registry::ToolRegistry;

/// 运行配置。
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// 最大步数。
    pub max_steps: usize,
    /// 工具超时。
    pub tool_timeout: Duration,
    /// 模型超时。
    pub model_timeout: Duration,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            max_steps: 8,
            tool_timeout: Duration::from_secs(5),
            model_timeout: Duration::from_secs(60),
        }
    }
}

/// 一次运行请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    /// 目标 Agent。
    pub agent_id: AgentId,
    /// 续用已有会话。
    pub session_id: Option<SessionId>,
    /// 用户输入内容块。
    pub content: Vec<ContentBlock>,
}

impl RunRequest {
    /// 以纯文本构造请求。
    pub fn new(agent_id: AgentId, text: impl Into<String>) -> Self {
        Self {
            agent_id,
            session_id: None,
            content: vec![ContentBlock::text(text)],
        }
    }

    /// 以内容块构造请求。
    pub fn with_content(
        agent_id: AgentId,
        session_id: Option<SessionId>,
        content: Vec<ContentBlock>,
    ) -> Self {
        Self {
            agent_id,
            session_id,
            content,
        }
    }

    /// 校验请求是否可以进入运行。
    pub fn validate(&self) -> Result<(), Error> {
        if self.content.is_empty() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "message content must not be empty",
            ));
        }
        if self
            .content
            .iter()
            .all(|block| block.as_text().trim().is_empty())
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "message text must not be empty",
            ));
        }
        Ok(())
    }
}

/// 工具调用记录。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallRecord {
    /// 调用 ID。
    pub tool_call_id: String,
    /// 工具名。
    pub name: String,
    /// 最终状态。
    pub status: ToolCallStatus,
    /// 参数。
    pub arguments: Option<JsonValue>,
    /// 结果。
    pub result: Option<JsonValue>,
    /// 错误。
    pub error: Option<Error>,
}

/// 运行结果。
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    /// 运行 ID。
    pub run_id: RunId,
    /// 会话 ID。
    pub session_id: SessionId,
    /// 助手文本。
    pub text: String,
    /// 工具调用记录。
    pub tool_calls: Vec<ToolCallRecord>,
    /// 终止原因。
    pub finish_reason: FinishReason,
    /// 终态。
    pub status: RunStatus,
    /// 运行错误。
    pub error: Option<Error>,
    /// 用量。
    pub usage: Option<Usage>,
}

/// Agent 运行服务。
pub struct AgentService {
    model: Arc<dyn ModelPort>,
    tools: Arc<ToolRegistry>,
    sessions: Arc<dyn SessionRepository>,
    runs: Arc<RunRegistry>,
    config: RunnerConfig,
}

impl AgentService {
    /// 装配运行服务。
    pub fn new(
        model: Arc<dyn ModelPort>,
        tools: Arc<ToolRegistry>,
        sessions: Arc<dyn SessionRepository>,
        runs: Arc<RunRegistry>,
        config: RunnerConfig,
    ) -> Self {
        Self {
            model,
            tools,
            sessions,
            runs,
            config,
        }
    }

    /// 模型名称。
    pub fn model_name(&self) -> &'static str {
        self.model.name()
    }

    /// 工具数量。
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// 运行注册表。
    pub fn runs(&self) -> &Arc<RunRegistry> {
        &self.runs
    }

    /// 执行一次运行，并将事件写入 sink。
    pub async fn run(
        &self,
        request: RunRequest,
        sink: &dyn EventSink,
        trace_id: Option<String>,
    ) -> Result<RunOutcome, Error> {
        request.validate()?;

        let session_id = match &request.session_id {
            Some(id) => id.clone(),
            None => new_session_id(),
        };
        let (run_id, cancel, status) = self.runs.register(&request.agent_id, &session_id)?;
        let registration = self.runs.attach(
            request.agent_id.clone(),
            run_id.clone(),
            session_id.clone(),
            cancel,
            status,
        );
        registration.set_status(RunStatus::Running);
        let mut signal = registration.signal();

        let mut session = match self.sessions.load(&session_id)? {
            Some(session) => session,
            None => Session::new(session_id.clone()),
        };
        session.active_run = Some(run_id.clone());

        let mut messages = session.messages.clone();
        messages.push(ChatMessage {
            role: Role::User,
            content: request.content.clone(),
        });

        let message_id = next_message_id();
        let mut text = String::new();
        let mut records: Vec<ToolCallRecord> = Vec::new();
        let mut duplicates: HashMap<String, u32> = HashMap::new();
        let mut finish_reason = FinishReason::Length;

        for step in 1..=self.config.max_steps {
            if *signal.borrow() {
                finish_reason = FinishReason::Cancelled;
                registration.set_status(RunStatus::Cancelled);
                emit_done(sink, finish_reason, None);
                persist(self.sessions.as_ref(), &mut session, &messages);
                return Ok(outcome(
                    run_id,
                    session_id,
                    text,
                    records,
                    finish_reason,
                    RunStatus::Cancelled,
                    None,
                    None,
                ));
            }

            let model_request = ModelRequest {
                messages: messages.clone(),
                tools: self.tools.descriptors(),
            };
            let model_future = self.model.step(model_request);
            let step_result = tokio::select! {
                changed = signal.changed() => {
                    let _ = changed;
                    None
                }
                timed = tokio::time::timeout(self.config.model_timeout, model_future) => Some(timed),
            };

            let Some(step_result) = step_result else {
                finish_reason = FinishReason::Cancelled;
                registration.set_status(RunStatus::Cancelled);
                emit_done(sink, finish_reason, None);
                persist(self.sessions.as_ref(), &mut session, &messages);
                return Ok(outcome(
                    run_id,
                    session_id,
                    text,
                    records,
                    finish_reason,
                    RunStatus::Cancelled,
                    None,
                    None,
                ));
            };

            let model_step = match step_result {
                Err(_) => {
                    let error = Error::new(ErrorCode::Timeout, "model step timed out");
                    registration.set_status(RunStatus::Failed);
                    sink.emit(RunEvent::Error {
                        error: error.clone(),
                        trace_id: trace_id.clone(),
                    });
                    persist(self.sessions.as_ref(), &mut session, &messages);
                    return Ok(outcome(
                        run_id,
                        session_id,
                        text,
                        records,
                        FinishReason::Error,
                        RunStatus::Failed,
                        Some(error),
                        None,
                    ));
                }
                Ok(Err(error)) => {
                    registration.set_status(RunStatus::Failed);
                    sink.emit(RunEvent::Error {
                        error: error.clone(),
                        trace_id: trace_id.clone(),
                    });
                    persist(self.sessions.as_ref(), &mut session, &messages);
                    return Ok(outcome(
                        run_id,
                        session_id,
                        text,
                        records,
                        FinishReason::Error,
                        RunStatus::Failed,
                        Some(error),
                        None,
                    ));
                }
                Ok(Ok(step)) => step,
            };

            match model_step {
                ModelStep::Text { delta } => {
                    text.push_str(&delta);
                    sink.emit(RunEvent::Chunk {
                        message_id: message_id.clone(),
                        delta: delta.clone(),
                    });
                    messages.push(ChatMessage::text(Role::Assistant, delta));
                    finish_reason = FinishReason::Stop;
                    break;
                }
                ModelStep::ToolCalls { calls, text: lead } => {
                    if let Some(lead) = lead {
                        text.push_str(&lead);
                        sink.emit(RunEvent::Chunk {
                            message_id: message_id.clone(),
                            delta: lead.clone(),
                        });
                        messages.push(ChatMessage::text(Role::Assistant, lead));
                    }

                    for call in calls {
                        let key = format!("{}:{}", call.name, call.arguments);
                        let count = duplicates.entry(key).or_insert(0);
                        *count += 1;
                        if *count > 2 {
                            let error = Error::new(
                                ErrorCode::Conflict,
                                format!("repeated tool call detected: {}", call.name),
                            );
                            registration.set_status(RunStatus::Failed);
                            sink.emit(RunEvent::Error {
                                error: error.clone(),
                                trace_id: trace_id.clone(),
                            });
                            persist(self.sessions.as_ref(), &mut session, &messages);
                            return Ok(outcome(
                                run_id,
                                session_id,
                                text,
                                records,
                                FinishReason::Error,
                                RunStatus::Failed,
                                Some(error),
                                None,
                            ));
                        }

                        sink.emit(RunEvent::ToolCall {
                            tool_call_id: call.tool_call_id.clone(),
                            name: call.name.clone(),
                            status: ToolCallStatus::Started,
                            arguments: Some(call.arguments.clone()),
                            result: None,
                            error: None,
                        });

                        let execution = match self.tools.get(&call.name) {
                            Some(tool) => {
                                execute_tool(tool, call.arguments.clone(), self.config.tool_timeout)
                                    .await
                            }
                            None => Err(Error::new(
                                ErrorCode::InvalidRequest,
                                format!("unknown tool: {}", call.name),
                            )),
                        };

                        match execution {
                            Ok(result) => {
                                sink.emit(RunEvent::ToolCall {
                                    tool_call_id: call.tool_call_id.clone(),
                                    name: call.name.clone(),
                                    status: ToolCallStatus::Completed,
                                    arguments: Some(call.arguments.clone()),
                                    result: Some(result.clone()),
                                    error: None,
                                });
                                messages.push(ChatMessage::text(Role::Tool, result.to_string()));
                                records.push(ToolCallRecord {
                                    tool_call_id: call.tool_call_id.clone(),
                                    name: call.name.clone(),
                                    status: ToolCallStatus::Completed,
                                    arguments: Some(call.arguments.clone()),
                                    result: Some(result),
                                    error: None,
                                });
                            }
                            Err(error) => {
                                sink.emit(RunEvent::ToolCall {
                                    tool_call_id: call.tool_call_id.clone(),
                                    name: call.name.clone(),
                                    status: ToolCallStatus::Failed,
                                    arguments: Some(call.arguments.clone()),
                                    result: None,
                                    error: Some(error.clone()),
                                });
                                messages.push(ChatMessage::text(
                                    Role::Tool,
                                    format!("tool error: {}", error.message()),
                                ));
                                records.push(ToolCallRecord {
                                    tool_call_id: call.tool_call_id.clone(),
                                    name: call.name.clone(),
                                    status: ToolCallStatus::Failed,
                                    arguments: Some(call.arguments.clone()),
                                    result: None,
                                    error: Some(error),
                                });
                            }
                        }
                    }

                    if step == self.config.max_steps {
                        finish_reason = FinishReason::Length;
                    }
                }
            }
        }

        registration.set_status(RunStatus::Completed);
        emit_done(sink, finish_reason, None);
        persist(self.sessions.as_ref(), &mut session, &messages);
        Ok(outcome(
            run_id,
            session_id,
            text,
            records,
            finish_reason,
            RunStatus::Completed,
            None,
            None,
        ))
    }

    /// 取消指定运行。
    pub fn cancel(&self, run_id: Option<&RunId>, agent_id: &AgentId) -> Result<RunStatus, Error> {
        match run_id {
            Some(run_id) => self.runs.cancel_run(run_id),
            None => self.runs.cancel_agent(agent_id),
        }
    }
}

async fn execute_tool(
    tool: Arc<dyn Tool>,
    arguments: JsonValue,
    timeout: Duration,
) -> Result<JsonValue, Error> {
    let join = tokio::task::spawn_blocking(move || tool.call(arguments));
    match tokio::time::timeout(timeout, join).await {
        Err(_) => Err(Error::new(ErrorCode::Timeout, "tool execution timed out")),
        Ok(Err(error)) => Err(Error::new(
            ErrorCode::Internal,
            format!("tool task failed: {error}"),
        )),
        Ok(Ok(result)) => result,
    }
}

fn emit_done(sink: &dyn EventSink, finish_reason: FinishReason, usage: Option<Usage>) {
    sink.emit(RunEvent::Done {
        finish_reason,
        usage,
    });
}

fn persist(sessions: &dyn SessionRepository, session: &mut Session, messages: &[ChatMessage]) {
    session.messages = messages.to_vec();
    session.active_run = None;
    let _ = sessions.save(session);
}

#[allow(clippy::too_many_arguments)]
fn outcome(
    run_id: RunId,
    session_id: SessionId,
    text: String,
    tool_calls: Vec<ToolCallRecord>,
    finish_reason: FinishReason,
    status: RunStatus,
    error: Option<Error>,
    usage: Option<Usage>,
) -> RunOutcome {
    RunOutcome {
        run_id,
        session_id,
        text,
        tool_calls,
        finish_reason,
        status,
        error,
        usage,
    }
}

fn next_message_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("msg-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn new_session_id() -> SessionId {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    SessionId::parse(format!("sess-{millis}-{counter}")).expect("generated session id is valid")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use dsh_domain::{BoxFuture, ModelRequest, ModelStep, Role};
    use dsh_infra::{ClockTool, EchoTool, FakeModel, FakeScript, InMemorySessionRepository};
    use dsh_types::{Error, ErrorCode, JsonValue, ToolCallRequest};

    use super::*;
    use crate::event_sink::CollectSink;
    use crate::tool_registry::ToolRegistry;

    fn registry() -> Arc<ToolRegistry> {
        let mut registry = ToolRegistry::default();
        registry.register(Arc::new(EchoTool));
        registry.register(Arc::new(ClockTool));
        Arc::new(registry)
    }

    fn service(
        model: Arc<dyn ModelPort>,
        max_steps: usize,
    ) -> (Arc<AgentService>, Arc<InMemorySessionRepository>) {
        let sessions = Arc::new(InMemorySessionRepository::default());
        let runs = Arc::new(RunRegistry::default());
        let service = AgentService::new(
            model,
            registry(),
            sessions.clone(),
            runs,
            RunnerConfig {
                max_steps,
                tool_timeout: Duration::from_secs(2),
                model_timeout: Duration::from_secs(2),
            },
        );
        (Arc::new(service), sessions)
    }

    fn agent() -> AgentId {
        AgentId::parse("agent-1").expect("valid")
    }

    #[tokio::test]
    async fn echo_model_finishes_with_stop() {
        let (service, _sessions) = service(Arc::new(FakeModel::new(FakeScript::Echo)), 4);
        let sink = CollectSink::default();
        let outcome = service
            .run(RunRequest::new(agent(), "hi"), &sink, None)
            .await
            .expect("run");
        assert_eq!(outcome.text, "echo: hi");
        assert_eq!(outcome.finish_reason, FinishReason::Stop);
        assert_eq!(outcome.status, RunStatus::Completed);
        let events = sink.events();
        assert!(matches!(events.last(), Some(RunEvent::Done { .. })));
    }

    #[tokio::test]
    async fn tool_script_executes_clock_and_summarizes() {
        let (service, _sessions) = service(Arc::new(FakeModel::new(FakeScript::Tool)), 4);
        let sink = CollectSink::default();
        let outcome = service
            .run(RunRequest::new(agent(), "time"), &sink, None)
            .await
            .expect("run");
        assert_eq!(outcome.tool_calls.len(), 1);
        assert_eq!(outcome.tool_calls[0].name, "clock");
        assert_eq!(outcome.tool_calls[0].status, ToolCallStatus::Completed);
        assert!(outcome.text.starts_with("observed: "));
        let events = sink.events();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, RunEvent::ToolCall { .. }))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn step_limit_finishes_with_length() {
        struct InfiniteToolModel {
            counter: AtomicUsize,
        }

        impl ModelPort for InfiniteToolModel {
            fn step<'a>(
                &'a self,
                _request: ModelRequest,
            ) -> BoxFuture<'a, Result<ModelStep, Error>> {
                let index = self.counter.fetch_add(1, Ordering::Relaxed);
                Box::pin(async move {
                    Ok(ModelStep::ToolCalls {
                        calls: vec![ToolCallRequest {
                            tool_call_id: format!("call-{index}"),
                            name: "echo".to_owned(),
                            arguments: serde_json::json!({"text": index.to_string()}),
                        }],
                        text: None,
                    })
                })
            }

            fn name(&self) -> &'static str {
                "infinite"
            }
        }

        let model = Arc::new(InfiniteToolModel {
            counter: AtomicUsize::new(0),
        });
        let (service, _sessions) = service(model, 3);
        let outcome = service
            .run(
                RunRequest::new(agent(), "loop"),
                &CollectSink::default(),
                None,
            )
            .await
            .expect("run");
        assert_eq!(outcome.finish_reason, FinishReason::Length);
        assert_eq!(outcome.status, RunStatus::Completed);
        assert_eq!(outcome.tool_calls.len(), 3);
    }

    #[tokio::test]
    async fn repeated_tool_call_is_rejected() {
        struct StuckModel;

        impl ModelPort for StuckModel {
            fn step<'a>(
                &'a self,
                _request: ModelRequest,
            ) -> BoxFuture<'a, Result<ModelStep, Error>> {
                Box::pin(async move {
                    Ok(ModelStep::ToolCalls {
                        calls: vec![ToolCallRequest {
                            tool_call_id: "call-same".to_owned(),
                            name: "echo".to_owned(),
                            arguments: serde_json::json!({"text": "same"}),
                        }],
                        text: None,
                    })
                })
            }

            fn name(&self) -> &'static str {
                "stuck"
            }
        }

        let (service, _sessions) = service(Arc::new(StuckModel), 6);
        let outcome = service
            .run(
                RunRequest::new(agent(), "loop"),
                &CollectSink::default(),
                None,
            )
            .await
            .expect("run");
        assert_eq!(outcome.status, RunStatus::Failed);
        assert_eq!(
            outcome.error.as_ref().map(Error::code),
            Some(ErrorCode::Conflict)
        );
    }

    #[tokio::test]
    async fn cancel_interrupts_running_model() {
        struct SlowModel;

        impl ModelPort for SlowModel {
            fn step<'a>(
                &'a self,
                _request: ModelRequest,
            ) -> BoxFuture<'a, Result<ModelStep, Error>> {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Ok(ModelStep::Text {
                        delta: "late".to_owned(),
                    })
                })
            }

            fn name(&self) -> &'static str {
                "slow"
            }
        }

        let (service, _sessions) = service(Arc::new(SlowModel), 2);
        let sink = Arc::new(CollectSink::default());
        let running = {
            let service = Arc::clone(&service);
            let sink = Arc::clone(&sink);
            tokio::spawn(async move {
                service
                    .run(RunRequest::new(agent(), "wait"), sink.as_ref(), None)
                    .await
            })
        };

        for _ in 0..100 {
            if !service.runs().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let status = service.cancel(None, &agent()).expect("cancel");
        assert_eq!(status, RunStatus::Running);
        let outcome = running.await.expect("join").expect("run");
        assert_eq!(outcome.finish_reason, FinishReason::Cancelled);
        assert_eq!(outcome.status, RunStatus::Cancelled);
        assert!(sink.events().iter().any(|event| matches!(
            event,
            RunEvent::Done {
                finish_reason: FinishReason::Cancelled,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn cancel_is_idempotent_after_completion() {
        struct SlowModel;

        impl ModelPort for SlowModel {
            fn step<'a>(
                &'a self,
                _request: ModelRequest,
            ) -> BoxFuture<'a, Result<ModelStep, Error>> {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    Ok(ModelStep::Text {
                        delta: "late".to_owned(),
                    })
                })
            }

            fn name(&self) -> &'static str {
                "slow"
            }
        }

        let (service, _sessions) = service(Arc::new(SlowModel), 2);
        let running = {
            let service = Arc::clone(&service);
            tokio::spawn(async move {
                service
                    .run(
                        RunRequest::new(agent(), "wait"),
                        &CollectSink::default(),
                        None,
                    )
                    .await
            })
        };
        for _ in 0..100 {
            if !service.runs().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            service.cancel(None, &agent()).expect("cancel"),
            RunStatus::Running
        );
        let outcome = running.await.expect("join").expect("run");
        assert_eq!(outcome.status, RunStatus::Cancelled);
        assert_eq!(
            service
                .cancel(Some(&outcome.run_id), &agent())
                .expect("repeat cancel"),
            RunStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn tool_directory_is_sorted() {
        let registry = registry();
        let names: Vec<String> = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.name)
            .collect();
        assert_eq!(names, vec!["clock".to_owned(), "echo".to_owned()]);
    }

    #[tokio::test]
    async fn session_history_is_reused() {
        let (service, sessions) = service(Arc::new(FakeModel::new(FakeScript::Echo)), 2);
        let first = service
            .run(
                RunRequest::new(agent(), "one"),
                &CollectSink::default(),
                None,
            )
            .await
            .expect("first");
        let second_request = RunRequest::with_content(
            agent(),
            Some(first.session_id.clone()),
            vec![ContentBlock::text("two")],
        );
        let second = service
            .run(second_request, &CollectSink::default(), None)
            .await
            .expect("second");
        assert_eq!(first.session_id, second.session_id);
        let session = sessions
            .load(&second.session_id)
            .expect("load")
            .expect("session");
        assert_eq!(session.messages.len(), 4);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[1].role, Role::Assistant);
    }

    #[tokio::test]
    async fn concurrent_session_is_rejected() {
        struct SlowModel;

        impl ModelPort for SlowModel {
            fn step<'a>(
                &'a self,
                _request: ModelRequest,
            ) -> BoxFuture<'a, Result<ModelStep, Error>> {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    Ok(ModelStep::Text {
                        delta: "done".to_owned(),
                    })
                })
            }

            fn name(&self) -> &'static str {
                "slow"
            }
        }

        let (service, _sessions) = service(Arc::new(SlowModel), 2);
        let session = SessionId::parse("sess-shared").expect("valid");
        fn make_request(session: &SessionId) -> RunRequest {
            RunRequest::with_content(
                agent(),
                Some(session.clone()),
                vec![ContentBlock::text("x")],
            )
        }

        let first = {
            let service = Arc::clone(&service);
            let session = session.clone();
            tokio::spawn(async move {
                service
                    .run(make_request(&session), &CollectSink::default(), None)
                    .await
            })
        };
        for _ in 0..100 {
            if !service.runs().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let error = service
            .run(make_request(&session), &CollectSink::default(), None)
            .await
            .expect_err("conflict");
        assert_eq!(error.code(), ErrorCode::Conflict);
        first.await.expect("join").expect("first run");
    }

    #[tokio::test]
    async fn unknown_tool_is_reported_as_failed() {
        struct UnknownToolModel;

        impl ModelPort for UnknownToolModel {
            fn step<'a>(
                &'a self,
                _request: ModelRequest,
            ) -> BoxFuture<'a, Result<ModelStep, Error>> {
                Box::pin(async move {
                    Ok(ModelStep::ToolCalls {
                        calls: vec![ToolCallRequest {
                            tool_call_id: "call-x".to_owned(),
                            name: "missing".to_owned(),
                            arguments: JsonValue::Object(serde_json::Map::new()),
                        }],
                        text: None,
                    })
                })
            }

            fn name(&self) -> &'static str {
                "unknown"
            }
        }

        let (service, _sessions) = service(Arc::new(UnknownToolModel), 2);
        let sink = CollectSink::default();
        let outcome = service
            .run(RunRequest::new(agent(), "x"), &sink, None)
            .await
            .expect("run");
        assert_eq!(outcome.tool_calls[0].status, ToolCallStatus::Failed);
        assert_eq!(
            outcome.tool_calls[0].error.as_ref().map(Error::code),
            Some(ErrorCode::InvalidRequest)
        );
    }
}
