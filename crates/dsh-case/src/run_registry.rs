//! 运行注册、取消广播与会话占用。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use dsh_domain::RunStatus;
use dsh_types::{AgentId, Error, ErrorCode, RunId, SessionId};
use tokio::sync::watch;

/// 单个运行的句柄。
struct RunHandle {
    agent_id: AgentId,
    cancel: watch::Sender<bool>,
    status: Arc<Mutex<RunStatus>>,
}

/// 运行注册返回值：运行 ID、取消发送端与共享状态。
pub type RunHandleParts = (RunId, watch::Sender<bool>, Arc<Mutex<RunStatus>>);

/// 运行注册表。
#[derive(Default)]
pub struct RunRegistry {
    runs: Mutex<HashMap<RunId, RunHandle>>,
    sessions: Mutex<HashSet<SessionId>>,
    finished: Mutex<HashMap<RunId, (AgentId, RunStatus)>>,
    last_agent_run: Mutex<HashMap<AgentId, RunId>>,
}

/// 运行登记凭证，Drop 时自动清理并释放会话占用。
pub struct RunRegistration {
    run_id: RunId,
    agent_id: AgentId,
    session_id: SessionId,
    signal: watch::Receiver<bool>,
    status: Arc<Mutex<RunStatus>>,
    registry: Arc<RunRegistry>,
}

impl RunRegistration {
    /// 运行 ID。
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// 取消信号。
    pub fn signal(&self) -> watch::Receiver<bool> {
        self.signal.clone()
    }

    /// 当前状态。
    pub fn status(&self) -> RunStatus {
        *self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// 更新状态。
    pub fn set_status(&self, next: RunStatus) {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *status = next;
    }
}

impl Drop for RunRegistration {
    fn drop(&mut self) {
        let status = self.status();
        if let Ok(mut runs) = self.registry.runs.lock() {
            runs.remove(&self.run_id);
        }
        if let Ok(mut sessions) = self.registry.sessions.lock() {
            sessions.remove(&self.session_id);
        }
        if let Ok(mut finished) = self.registry.finished.lock() {
            if finished.len() > 1024 {
                finished.clear();
            }
            finished.insert(self.run_id.clone(), (self.agent_id.clone(), status));
        }
        if let Ok(mut last) = self.registry.last_agent_run.lock() {
            last.insert(self.agent_id.clone(), self.run_id.clone());
        }
    }
}

impl RunRegistry {
    /// 登记运行并占用会话；会话已被占用时返回 `conflict`。
    pub fn register(
        &self,
        agent_id: &AgentId,
        session_id: &SessionId,
    ) -> Result<RunHandleParts, Error> {
        {
            let mut sessions = self
                .sessions
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if !sessions.insert(session_id.clone()) {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    format!("session {session_id} already has a running agent"),
                ));
            }
        }
        let run_id = new_run_id();
        let (cancel, _) = watch::channel(false);
        let status = Arc::new(Mutex::new(RunStatus::Pending));
        self.runs
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                run_id.clone(),
                RunHandle {
                    agent_id: agent_id.clone(),
                    cancel: cancel.clone(),
                    status: Arc::clone(&status),
                },
            );
        Ok((run_id, cancel, status))
    }

    /// 构造登记凭证。
    pub fn attach(
        self: &Arc<Self>,
        agent_id: AgentId,
        run_id: RunId,
        session_id: SessionId,
        cancel: watch::Sender<bool>,
        status: Arc<Mutex<RunStatus>>,
    ) -> RunRegistration {
        RunRegistration {
            run_id,
            agent_id,
            session_id,
            signal: cancel.subscribe(),
            status,
            registry: Arc::clone(self),
        }
    }

    /// 按运行 ID 取消。
    pub fn cancel_run(&self, run_id: &RunId) -> Result<RunStatus, Error> {
        {
            let runs = self
                .runs
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if let Some(handle) = runs.get(run_id) {
                let _ = handle.cancel.send(true);
                return Ok(*handle
                    .status
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner()));
            }
        }
        self.finished
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(run_id)
            .map(|(_, status)| *status)
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("run not found: {run_id}")))
    }

    /// 按 Agent 取消其当前运行；无活动运行时返回最近一次运行的终态。
    pub fn cancel_agent(&self, agent_id: &AgentId) -> Result<RunStatus, Error> {
        {
            let runs = self
                .runs
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if let Some(handle) = runs.values().find(|handle| &handle.agent_id == agent_id) {
                let _ = handle.cancel.send(true);
                return Ok(*handle
                    .status
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner()));
            }
        }
        let last_run = self
            .last_agent_run
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(agent_id)
            .cloned()
            .ok_or_else(|| {
                Error::new(ErrorCode::NotFound, format!("no running agent: {agent_id}"))
            })?;
        self.finished
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(&last_run)
            .map(|(_, status)| *status)
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("no running agent: {agent_id}")))
    }

    /// 当前运行数量。
    pub fn len(&self) -> usize {
        self.runs.lock().map(|runs| runs.len()).unwrap_or(0)
    }

    /// 是否没有运行。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 生成单进程唯一的运行 ID。
pub fn new_run_id() -> RunId {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    RunId::parse(format!("run-{millis}-{counter}")).expect("generated run id is valid")
}
