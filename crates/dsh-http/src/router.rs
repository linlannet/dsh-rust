//! Axum 路由与处理器。

use std::future::Future;
use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dsh_case::{AgentService, CollectSink, EventSink, RunRequest};
use dsh_domain::RunEvent;

use crate::dto::{
    AgentCancelRequest, AgentCancelResponse, AgentMessageRequest, AgentMessageResponse,
    HealthResponse, SessionEventsQuery, SessionEventsResponse,
};
use crate::error::HttpError;
use crate::sse;
use crate::trace::new_trace_id;

/// 回放分页默认条数。
const EVENTS_DEFAULT_LIMIT: i64 = 200;
/// 回放分页上限。
const EVENTS_MAX_LIMIT: i64 = 1000;

/// 路由共享状态。
#[derive(Clone)]
pub struct AppState {
    /// 运行服务。
    pub service: Arc<AgentService>,
    /// 单 run SSE 字节预算。
    pub stream_budget_bytes: usize,
}

impl AppState {
    /// 构造状态，预算取默认值。
    pub fn new(service: Arc<AgentService>) -> Self {
        Self {
            service,
            stream_budget_bytes: sse::DEFAULT_STREAM_BUDGET_BYTES,
        }
    }

    /// 覆盖单 run SSE 字节预算。
    pub fn with_stream_budget(mut self, bytes: usize) -> Self {
        self.stream_budget_bytes = bytes;
        self
    }
}

/// 构建路由。
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route(crate::routes::HEALTH, get(health))
        .route(crate::routes::AGENT_MESSAGE, post(message))
        .route(crate::routes::AGENT_STREAM, post(stream))
        .route(crate::routes::AGENT_CANCEL, post(cancel))
        .route(crate::routes::SESSIONS_EVENTS, get(session_events))
        .with_state(state)
}

async fn health(State(_state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        contract_version: dsh_types::CONTRACT_VERSION,
    })
}

async fn message(
    State(state): State<AppState>,
    payload: Result<Json<AgentMessageRequest>, JsonRejection>,
) -> Result<Json<AgentMessageResponse>, HttpError> {
    let trace_id = new_trace_id();
    let Json(request) = payload.map_err(|rejection| {
        HttpError::from(dsh_types::Error::new(
            dsh_types::ErrorCode::InvalidRequest,
            rejection.body_text(),
        ))
        .with_trace(Some(trace_id.clone()))
    })?;
    let sink = CollectSink::default();
    let run_request = RunRequest::with_content(
        request.agent_id.clone(),
        request.session_id.clone(),
        request.content.clone(),
    );
    let outcome = state
        .service
        .run(run_request, &sink, Some(trace_id.clone()))
        .await
        .map_err(|error| HttpError::from(error).with_trace(Some(trace_id.clone())))?;
    if let Some(error) = outcome.error.clone() {
        return Err(HttpError::from(error).with_trace(Some(trace_id)));
    }
    Ok(Json(AgentMessageResponse::from_outcome(
        &request.agent_id,
        outcome,
    )))
}

async fn stream(
    State(state): State<AppState>,
    payload: Result<Json<AgentMessageRequest>, JsonRejection>,
) -> Result<Response, HttpError> {
    let trace_id = new_trace_id();
    let Json(request) = payload.map_err(|rejection| {
        HttpError::from(dsh_types::Error::new(
            dsh_types::ErrorCode::InvalidRequest,
            rejection.body_text(),
        ))
        .with_trace(Some(trace_id.clone()))
    })?;
    let run_request = RunRequest::with_content(
        request.agent_id.clone(),
        request.session_id.clone(),
        request.content.clone(),
    );
    let (sink, receiver) = sse::channel();
    let service = Arc::clone(&state.service);
    let trace_for_task = trace_id.clone();
    tokio::spawn(async move {
        if let Err(error) = service
            .run(run_request, &sink, Some(trace_for_task.clone()))
            .await
        {
            sink.emit(RunEvent::Error {
                error,
                trace_id: Some(trace_for_task),
            });
        }
    });
    Ok(sse::sse_response_with_budget(receiver, state.stream_budget_bytes).into_response())
}

async fn cancel(
    State(state): State<AppState>,
    payload: Result<Json<AgentCancelRequest>, JsonRejection>,
) -> Result<Json<AgentCancelResponse>, HttpError> {
    let trace_id = new_trace_id();
    let Json(request) = payload.map_err(|rejection| {
        HttpError::from(dsh_types::Error::new(
            dsh_types::ErrorCode::InvalidRequest,
            rejection.body_text(),
        ))
        .with_trace(Some(trace_id.clone()))
    })?;
    let status = state
        .service
        .cancel(request.run_id.as_ref(), &request.agent_id)
        .map_err(|error| HttpError::from(error).with_trace(Some(trace_id)))?;
    Ok(Json(AgentCancelResponse::new(
        &request.agent_id,
        request.run_id,
        status,
    )))
}

/// 会话事件回放：按 `seq` 升序分页。
async fn session_events(
    State(state): State<AppState>,
    session_id: Result<Path<dsh_types::SessionId>, PathRejection>,
    query: Result<Query<SessionEventsQuery>, QueryRejection>,
) -> Result<Json<SessionEventsResponse>, HttpError> {
    let trace_id = new_trace_id();
    let Path(session_id) =
        session_id.map_err(|rejection| invalid_request(rejection.body_text(), &trace_id))?;
    let Query(query) =
        query.map_err(|rejection| invalid_request(rejection.body_text(), &trace_id))?;

    let after_seq = match query.after_seq {
        None => 0,
        Some(value) if value >= 0 => value as u64,
        Some(value) => {
            return Err(invalid_request(
                format!("afterSeq must be zero or positive, got {value}"),
                &trace_id,
            ));
        }
    };
    let limit = match query.limit {
        None => EVENTS_DEFAULT_LIMIT as usize,
        Some(value) if (1..=EVENTS_MAX_LIMIT).contains(&value) => value as usize,
        Some(value) => {
            return Err(invalid_request(
                format!("limit must be between 1 and {EVENTS_MAX_LIMIT}, got {value}"),
                &trace_id,
            ));
        }
    };

    let events = state
        .service
        .session_events(&session_id, after_seq, limit)
        .await
        .map_err(|error| HttpError::from(error).with_trace(Some(trace_id)))?;
    let last_seq = events.last().map(|event| event.seq).unwrap_or(0);

    Ok(Json(SessionEventsResponse {
        session_id,
        events,
        last_seq,
    }))
}

/// 构造 `invalid_request` 错误，统一携带 traceId。
fn invalid_request(message: impl Into<String>, trace_id: &str) -> HttpError {
    HttpError::from(dsh_types::Error::new(
        dsh_types::ErrorCode::InvalidRequest,
        message.into(),
    ))
    .with_trace(Some(trace_id.to_owned()))
}

/// 启动服务并在 `shutdown` 完成时优雅关闭。
pub async fn serve(
    addr: &str,
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
}
