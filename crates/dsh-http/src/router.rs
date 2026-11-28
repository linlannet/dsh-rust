//! Axum 路由与处理器。

use std::future::Future;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dsh_case::{AgentService, CollectSink, EventSink, RunRequest};
use dsh_domain::RunEvent;

use crate::dto::{
    AgentCancelRequest, AgentCancelResponse, AgentMessageRequest, AgentMessageResponse,
    HealthResponse,
};
use crate::error::HttpError;
use crate::sse;
use crate::trace::new_trace_id;

/// 路由共享状态。
#[derive(Clone)]
pub struct AppState {
    /// 运行服务。
    pub service: Arc<AgentService>,
}

impl AppState {
    /// 构造状态。
    pub fn new(service: Arc<AgentService>) -> Self {
        Self { service }
    }
}

/// 构建路由。
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route(crate::routes::HEALTH, get(health))
        .route(crate::routes::AGENT_MESSAGE, post(message))
        .route(crate::routes::AGENT_STREAM, post(stream))
        .route(crate::routes::AGENT_CANCEL, post(cancel))
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
    Ok(sse::sse_response(receiver).into_response())
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
