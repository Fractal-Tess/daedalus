use std::convert::Infallible;

use async_stream::stream;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use daedalus_core::DaedalusError;
use daedalus_domain::{ConfigUpdateResponse, DownloadRequest, ImportRequest, SearchResult};
use daedalus_service::DaedalusService;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ApiState {
    service: DaedalusService,
}

impl ApiState {
    pub fn new(service: DaedalusService) -> Self {
        Self { service }
    }
}

pub fn router(service: DaedalusService) -> Router {
    let state = ApiState::new(service);
    Router::new()
        .route("/health", get(health))
        .route("/config", get(get_config).put(update_config))
        .route("/library/items", get(list_library_items))
        .route("/library/items/{id}", get(get_library_item))
        .route("/library/import", post(import_library_item))
        .route("/library/rescan", post(rescan_library))
        .route("/sources", get(list_sources))
        .route("/sources/civitai/search", get(civitai_search))
        .route("/sources/civitai/models/{id}", get(civitai_model_detail))
        .route("/downloads", post(queue_download))
        .route("/downloads/{id}/cancel", post(cancel_download))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(get_job))
        .route("/events", get(events))
        .with_state(state)
}

async fn health(State(state): State<ApiState>) -> ApiResult<Json<daedalus_domain::ServiceHealth>> {
    Ok(Json(state.service.health()?))
}

async fn get_config(State(state): State<ApiState>) -> ApiResult<Json<daedalus_config::AppConfig>> {
    Ok(Json(state.service.current_config()?))
}

async fn update_config(
    State(state): State<ApiState>,
    Json(config): Json<daedalus_config::AppConfig>,
) -> ApiResult<Json<ConfigUpdateResponse>> {
    state.service.update_config(config)?;
    Ok(Json(ConfigUpdateResponse {
        saved_to: state.service.config_store().path().display().to_string(),
    }))
}

async fn list_library_items(State(state): State<ApiState>) -> ApiResult<Json<Vec<daedalus_domain::LibraryItem>>> {
    Ok(Json(state.service.list_library_items()?))
}

async fn get_library_item(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<daedalus_domain::LibraryItem>> {
    match state.service.get_library_item(id)? {
        Some(item) => Ok(Json(item)),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, format!("library item {id} not found"))),
    }
}

async fn import_library_item(
    State(state): State<ApiState>,
    Json(request): Json<ImportRequest>,
) -> ApiResult<Json<daedalus_domain::LibraryItem>> {
    Ok(Json(state.service.import_local_file(request)?))
}

async fn rescan_library(State(state): State<ApiState>) -> ApiResult<Json<daedalus_domain::Job>> {
    Ok(Json(state.service.rescan_library()?))
}

async fn list_sources(State(state): State<ApiState>) -> ApiResult<Json<Vec<daedalus_domain::SourceInfo>>> {
    Ok(Json(state.service.list_sources()?))
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct NotReadyResponse {
    detail: String,
}

async fn civitai_search(Query(params): Query<SearchParams>) -> ApiResult<Json<SearchResult<daedalus_domain::ModelSummary>>> {
    let _query = params.q.trim();
    let _limit = params.limit.unwrap_or(20);
    Ok(Json(SearchResult {
        items: Vec::new(),
        total: 0,
        next_page: None,
    }))
}

async fn civitai_model_detail(Path(id): Path<String>) -> ApiResult<Json<NotReadyResponse>> {
    Err(ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        format!("Civitai model detail is not implemented yet for id {id}"),
    ))
}

async fn queue_download(
    State(state): State<ApiState>,
    Json(request): Json<DownloadRequest>,
) -> ApiResult<Json<daedalus_domain::Job>> {
    Ok(Json(state.service.queue_download(request)?))
}

async fn cancel_download(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<daedalus_domain::Job>> {
    match state.service.cancel_job(id)? {
        Some(job) => Ok(Json(job)),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, format!("job {id} not found"))),
    }
}

async fn list_jobs(State(state): State<ApiState>) -> ApiResult<Json<Vec<daedalus_domain::Job>>> {
    Ok(Json(state.service.list_jobs()?))
}

async fn get_job(State(state): State<ApiState>, Path(id): Path<i64>) -> ApiResult<Json<daedalus_domain::Job>> {
    match state.service.get_job(id)? {
        Some(job) => Ok(Json(job)),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, format!("job {id} not found"))),
    }
}

async fn events(
    State(state): State<ApiState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let health = state.service.health().ok();
    let stream = stream! {
        if let Some(health) = health {
            let payload = serde_json::to_string(&health).unwrap_or_else(|_| "{\"status\":\"ok\"}".to_string());
            yield Ok(Event::default().event("bootstrap").data(payload));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: String) -> Self {
        Self { status, message }
    }
}

impl From<DaedalusError> for ApiError {
    fn from(value: DaedalusError) -> Self {
        let status = match value {
            DaedalusError::NotFound(_) => StatusCode::NOT_FOUND,
            DaedalusError::Validation(_) | DaedalusError::Config(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: value.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}
