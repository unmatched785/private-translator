use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderValue, Request, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    config::{AppConfig, ModelConfig},
    engine::{self, TranslateRequest},
    engine_manager::{EngineManager, EngineQueueStatus, TranslateError},
    history::{HistoryFilter, HistoryRecord, NewHistoryRecord},
    storage::StorageWorker,
    translation_memory::{NewMemoryRevision, TranslationMemoryRecord},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage: StorageWorker,
    pub engines: Arc<EngineManager>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles_css))
        .route("/api/config", get(get_config))
        .route("/api/health", get(get_health))
        .route("/api/translate", post(translate))
        .route("/api/history", get(list_history))
        .route("/api/history/{id}", get(get_history).delete(delete_history))
        .route("/api/history/{id}/favorite", patch(set_favorite))
        .route("/api/history/{id}/approve", post(approve_history))
        .route("/api/memory", get(list_memory))
        .route(
            "/api/memory/{id}",
            get(get_memory).put(revise_memory).delete(delete_memory),
        )
        .route("/api/memory/{id}/revisions", get(list_memory_revisions))
        .fallback(not_found)
        .layer(middleware::from_fn(local_security))
        .with_state(state)
}

async fn local_security(request: Request<Body>, next: Next) -> Response {
    let allowed_host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| {
            host.starts_with("127.0.0.1:")
                || host == "127.0.0.1"
                || host.starts_with("localhost:")
                || host == "localhost"
                || host.starts_with("[::1]:")
                || host == "[::1]"
        });

    let mut response = if allowed_host {
        next.run(request).await
    } else {
        ApiError::not_found("로컬 주소에서만 사용할 수 있습니다").into_response()
    };
    apply_security_headers(&mut response);
    response
}

async fn index() -> impl IntoResponse {
    secure_static(Html(include_str!("../web/index.html")))
}

async fn app_js() -> impl IntoResponse {
    secure_asset(
        "application/javascript; charset=utf-8",
        include_str!("../web/app.js"),
    )
}

async fn styles_css() -> impl IntoResponse {
    secure_asset("text/css; charset=utf-8", include_str!("../web/styles.css"))
}

async fn get_config(State(state): State<AppState>) -> Json<PublicConfig> {
    Json(PublicConfig::from(state.config.as_ref()))
}

async fn get_health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    let history_count = state.storage.count().await.map_err(ApiError::internal)?;
    Ok(Json(HealthResponse {
        status: "ok",
        profile: state.config.profile.clone(),
        vault: "encrypted",
        history_count,
        engine: state.engines.status(),
    }))
}

async fn translate(
    State(state): State<AppState>,
    Json(request): Json<TranslateRequest>,
) -> Result<Json<TranslateResponse>, ApiError> {
    let text = request.text.trim();
    if text.is_empty() {
        return Err(ApiError::bad_request("번역할 텍스트를 입력하세요"));
    }
    if request.text.chars().count() > state.config.max_text_chars {
        return Err(ApiError::bad_request(format!(
            "한 번에 번역할 수 있는 최대 길이는 {}자입니다",
            state.config.max_text_chars
        )));
    }
    if request.target == "auto" {
        return Err(ApiError::bad_request("대상 언어를 선택하세요"));
    }
    validate_request_identity(&request)?;

    let model = state
        .config
        .model(&request.model)
        .ok_or_else(|| ApiError::bad_request("선택한 모델이 현재 프로필에 없습니다"))?;
    let engine_result = state
        .engines
        .translate(&request)
        .await
        .map_err(|error| match error {
            TranslateError::UnknownModel => {
                ApiError::bad_request("선택한 모델이 현재 프로필에 없습니다")
            }
            TranslateError::Superseded => {
                ApiError::conflict("더 최신 번역 요청으로 교체되었습니다")
            }
            TranslateError::Failed(error) => ApiError::unavailable(error.to_string()),
        })?;

    let mut history_id = None;
    if request.save_history {
        let record = state
            .storage
            .insert(NewHistoryRecord {
                source_text: request.text.clone(),
                translated_text: engine_result.translated_text.clone(),
                source_lang: request.source.clone(),
                target_lang: request.target.clone(),
                model_id: model.id.clone(),
                model_label: model.label.clone(),
                mode: request.mode.clone(),
                privacy: engine::privacy_label(model.privacy).into(),
                latency_ms: engine_result.latency_ms,
                qa_warnings: engine_result.qa_warnings.clone(),
            })
            .await
            .map_err(ApiError::internal)?;
        history_id = Some(record.id);
    }

    Ok(Json(TranslateResponse {
        translated_text: engine_result.translated_text,
        source: request.source,
        target: request.target,
        model_id: model.id.clone(),
        model_label: model.label.clone(),
        privacy: engine::privacy_label(model.privacy).into(),
        latency_ms: engine_result.latency_ms,
        chunk_count: engine_result.chunk_count,
        history_id,
        qa_warnings: engine_result.qa_warnings,
    }))
}

fn validate_request_identity(request: &TranslateRequest) -> Result<(), ApiError> {
    match (&request.client_id, request.request_seq) {
        (None, None) => Ok(()),
        (Some(client_id), Some(sequence))
            if sequence > 0
                && (1..=64).contains(&client_id.len())
                && client_id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                }) =>
        {
            Ok(())
        }
        _ => Err(ApiError::bad_request(
            "번역 요청 식별자가 올바르지 않습니다",
        )),
    }
}

async fn list_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryListResponse>, ApiError> {
    let records = state
        .storage
        .list(HistoryFilter {
            query: query.q,
            favorites_only: query.favorites.unwrap_or(false),
            approved_only: query.approved.unwrap_or(false),
            limit: query.limit.unwrap_or(100),
        })
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(HistoryListResponse { records }))
}

async fn get_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<HistoryRecord>, ApiError> {
    state
        .storage
        .get(&id)
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("번역 기록을 찾을 수 없습니다"))
}

async fn set_favorite(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<FavoriteRequest>,
) -> Result<StatusCode, ApiError> {
    if state
        .storage
        .set_favorite(&id, request.favorite)
        .await
        .map_err(ApiError::internal)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("번역 기록을 찾을 수 없습니다"))
    }
}

async fn delete_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if state
        .storage
        .delete(&id)
        .await
        .map_err(ApiError::internal)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("번역 기록을 찾을 수 없습니다"))
    }
}

async fn approve_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TranslationMemoryRecord>, ApiError> {
    state
        .storage
        .approve_history(&id)
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("승인할 번역 기록을 찾을 수 없습니다"))
}

async fn list_memory(
    State(state): State<AppState>,
    Query(query): Query<MemoryQuery>,
) -> Result<Json<MemoryListResponse>, ApiError> {
    let records = state
        .storage
        .list_memory(query.limit.unwrap_or(100))
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MemoryListResponse { records }))
}

async fn get_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TranslationMemoryRecord>, ApiError> {
    state
        .storage
        .get_memory(&id)
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("번역 자산을 찾을 수 없습니다"))
}

async fn revise_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<MemoryRevisionRequest>,
) -> Result<Json<TranslationMemoryRecord>, ApiError> {
    if request.source_text.trim().is_empty() || request.translated_text.trim().is_empty() {
        return Err(ApiError::bad_request("원문과 번역문을 모두 입력하세요"));
    }
    if request.source_text.chars().count() > state.config.max_text_chars
        || request.translated_text.chars().count() > state.config.max_text_chars
    {
        return Err(ApiError::bad_request(format!(
            "번역 자산의 원문과 번역문은 각각 {}자 이하여야 합니다",
            state.config.max_text_chars
        )));
    }
    if request.target_lang == "auto" {
        return Err(ApiError::bad_request("대상 언어를 선택하세요"));
    }
    let qa_warnings = engine::qa_warnings(&request.source_text, &request.translated_text);
    state
        .storage
        .revise_memory(
            &id,
            NewMemoryRevision {
                source_text: request.source_text,
                translated_text: request.translated_text,
                source_lang: request.source_lang,
                target_lang: request.target_lang,
                qa_warnings,
            },
        )
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("수정할 번역 자산을 찾을 수 없습니다"))
}

async fn list_memory_revisions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MemoryListResponse>, ApiError> {
    if state
        .storage
        .get_memory(&id)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(ApiError::not_found("번역 자산을 찾을 수 없습니다"));
    }
    let records = state
        .storage
        .list_memory_revisions(&id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MemoryListResponse { records }))
}

async fn delete_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if state
        .storage
        .delete_memory(&id)
        .await
        .map_err(ApiError::internal)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("삭제할 번역 자산을 찾을 수 없습니다"))
    }
}

async fn not_found() -> impl IntoResponse {
    ApiError::not_found("요청한 경로를 찾을 수 없습니다")
}

fn secure_static<T: IntoResponse>(response: T) -> Response {
    let mut response = response.into_response();
    apply_security_headers(&mut response);
    response
}

fn secure_asset(content_type: &'static str, body: &'static str) -> Response {
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    apply_security_headers(&mut response);
    response
}

fn apply_security_headers(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
}

#[derive(Debug, Serialize)]
struct PublicConfig {
    profile: String,
    display_name: String,
    default_model: String,
    max_text_chars: usize,
    models: Vec<PublicModel>,
}

impl From<&AppConfig> for PublicConfig {
    fn from(config: &AppConfig) -> Self {
        Self {
            profile: config.profile.clone(),
            display_name: config.display_name.clone(),
            default_model: config.default_model.clone(),
            max_text_chars: config.max_text_chars,
            models: config.models.iter().map(PublicModel::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct PublicModel {
    id: String,
    label: String,
    description: String,
    privacy: String,
}

impl From<&ModelConfig> for PublicModel {
    fn from(model: &ModelConfig) -> Self {
        Self {
            id: model.id.clone(),
            label: model.label.clone(),
            description: model.description.clone(),
            privacy: engine::privacy_label(model.privacy).into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    profile: String,
    vault: &'static str,
    history_count: u64,
    engine: EngineQueueStatus,
}

#[derive(Debug, Serialize)]
struct TranslateResponse {
    translated_text: String,
    source: String,
    target: String,
    model_id: String,
    model_label: String,
    privacy: String,
    latency_ms: u64,
    chunk_count: usize,
    history_id: Option<String>,
    qa_warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    q: Option<String>,
    favorites: Option<bool>,
    approved: Option<bool>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct HistoryListResponse {
    records: Vec<HistoryRecord>,
}

#[derive(Debug, Deserialize)]
struct MemoryQuery {
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct MemoryListResponse {
    records: Vec<TranslationMemoryRecord>,
}

#[derive(Debug, Deserialize)]
struct MemoryRevisionRequest {
    source_text: String,
    translated_text: String,
    source_lang: String,
    target_lang: String,
}

#[derive(Debug, Deserialize)]
struct FavoriteRequest {
    favorite: bool,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorMessage,
}

#[derive(Debug, Serialize)]
struct ErrorMessage {
    message: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        eprintln!("internal error: {error}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "로컬 기록을 처리하는 중 오류가 발생했습니다".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: ErrorMessage {
                    message: self.message,
                },
            }),
        )
            .into_response();
        apply_security_headers(&mut response);
        response
    }
}
