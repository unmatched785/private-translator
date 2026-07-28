use std::{collections::HashMap, sync::Arc};

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
    pub security: Arc<SecurityContext>,
}

#[derive(Debug)]
pub struct SecurityContext {
    pub token: Arc<str>,
    pub expected_host: Arc<str>,
    pub expected_origin: Arc<str>,
}

impl SecurityContext {
    pub fn new(
        token: impl Into<Arc<str>>,
        expected_host: impl Into<Arc<str>>,
        expected_origin: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            token: token.into(),
            expected_host: expected_host.into(),
            expected_origin: expected_origin.into(),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let security = Arc::clone(&state.security);
    Router::new()
        .route("/", get(index))
        .route("/i18n.js", get(i18n_js))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles_css))
        .route("/api/config", get(get_config))
        .route("/api/health", get(get_health))
        .route("/api/translate", post(translate))
        .route("/api/translate/cancel", post(cancel_translation))
        .route("/api/history", get(list_history))
        .route("/api/history/search", post(search_history))
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
        .layer(middleware::from_fn_with_state(security, local_security))
        .with_state(state)
}

async fn local_security(
    State(security): State<Arc<SecurityContext>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let allowed_host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| constant_time_equal(host, &security.expected_host));
    let allowed_origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|origin| constant_time_equal(origin, &security.expected_origin));
    let is_api = request.uri().path().starts_with("/api/");
    let authorized = !is_api
        || request_token(&request).is_some_and(|token| constant_time_equal(token, &security.token));

    let mut response = if !allowed_host {
        ApiError::not_found(
            "local_only",
            "This app can only be used from its exact local address.",
        )
        .into_response()
    } else if !allowed_origin {
        ApiError::forbidden(
            "origin_rejected",
            "The request did not come from this local app.",
        )
        .into_response()
    } else if !authorized {
        ApiError::unauthorized(
            "session_required",
            "Relaunch Private Translator to open an authenticated local session.",
        )
        .into_response()
    } else {
        next.run(request).await
    };
    apply_security_headers(&mut response);
    response
}

fn request_token(request: &Request<Body>) -> Option<&str> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
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

async fn i18n_js() -> impl IntoResponse {
    secure_asset(
        "application/javascript; charset=utf-8",
        include_str!("../web/i18n.js"),
    )
}

async fn styles_css() -> impl IntoResponse {
    secure_asset("text/css; charset=utf-8", include_str!("../web/styles.css"))
}

async fn get_config(State(state): State<AppState>) -> Json<PublicConfig> {
    let availability = state.engines.availability().await;
    Json(PublicConfig::new(state.config.as_ref(), &availability))
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
        return Err(ApiError::bad_request(
            "empty_text",
            "Enter text to translate.",
        ));
    }
    if request.text.chars().count() > state.config.max_text_chars {
        return Err(ApiError::bad_request(
            "text_too_long",
            format!(
                "Text cannot exceed {} characters.",
                state.config.max_text_chars
            ),
        ));
    }
    if request.target == "auto" {
        return Err(ApiError::bad_request(
            "target_required",
            "Select a target language.",
        ));
    }
    validate_request_identity(&request)?;

    let model = state.config.model(&request.model).ok_or_else(|| {
        ApiError::bad_request(
            "model_not_found",
            "The selected model is not available in this profile.",
        )
    })?;
    let supported_languages = engine::supported_language_codes(model.family);
    let source_supported =
        request.source == "auto" || supported_languages.contains(&request.source.as_str());
    let target_supported = supported_languages.contains(&request.target.as_str());
    if !source_supported || !target_supported {
        return Err(ApiError::bad_request(
            "unsupported_language",
            "The selected model does not support one of these languages.",
        ));
    }
    let engine_result = state
        .engines
        .translate(&request)
        .await
        .map_err(|error| match error {
            TranslateError::UnknownModel => ApiError::bad_request(
                "model_not_found",
                "The selected model is not available in this profile.",
            ),
            TranslateError::Superseded => ApiError::conflict(
                "request_superseded",
                "A newer translation request replaced this one.",
            ),
            TranslateError::Failed(error) => ApiError::unavailable(error),
        })?;
    if !state.engines.is_current(&request) {
        return Err(ApiError::conflict(
            "request_superseded",
            "A newer translation request replaced this one.",
        ));
    }

    let mut history_id = None;
    let mut history_error = None;
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
            .await;
        match record {
            Ok(record) => history_id = Some(record.id),
            Err(error) => {
                eprintln!("history save failed after translation: {error}");
                history_error = Some("storage_error");
            }
        }
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
        history_error,
        qa_warnings: engine_result.qa_warnings,
    }))
}

async fn cancel_translation(
    State(state): State<AppState>,
    Json(request): Json<CancelTranslationRequest>,
) -> Result<StatusCode, ApiError> {
    validate_identity(&request.client_id, request.request_seq)?;
    state
        .engines
        .cancel(&request.client_id, request.request_seq);
    Ok(StatusCode::NO_CONTENT)
}

fn validate_request_identity(request: &TranslateRequest) -> Result<(), ApiError> {
    match (&request.client_id, request.request_seq) {
        (None, None) => Ok(()),
        (Some(client_id), Some(sequence)) => validate_identity(client_id, sequence),
        _ => Err(ApiError::bad_request(
            "invalid_request_id",
            "The translation request identifier is invalid.",
        )),
    }
}

fn validate_identity(client_id: &str, sequence: u64) -> Result<(), ApiError> {
    if sequence > 0
        && (1..=64).contains(&client_id.len())
        && client_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Ok(());
    }
    Err(ApiError::bad_request(
        "invalid_request_id",
        "The translation request identifier is invalid.",
    ))
}

async fn list_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryListResponse>, ApiError> {
    let records = state
        .storage
        .list(HistoryFilter {
            query: None,
            favorites_only: query.favorites.unwrap_or(false),
            approved_only: query.approved.unwrap_or(false),
            limit: query.limit.unwrap_or(100),
        })
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(HistoryListResponse { records }))
}

async fn search_history(
    State(state): State<AppState>,
    Json(request): Json<HistorySearchRequest>,
) -> Result<Json<HistoryListResponse>, ApiError> {
    let records = state
        .storage
        .list(HistoryFilter {
            query: request
                .query
                .map(|query| query.trim().to_owned())
                .filter(|query| !query.is_empty()),
            favorites_only: request.favorites.unwrap_or(false),
            approved_only: request.approved.unwrap_or(false),
            limit: request.limit.unwrap_or(100),
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
        .ok_or_else(|| {
            ApiError::not_found("history_not_found", "The translation record was not found.")
        })
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
        Err(ApiError::not_found(
            "history_not_found",
            "The translation record was not found.",
        ))
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
        Err(ApiError::not_found(
            "history_not_found",
            "The translation record was not found.",
        ))
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
        .ok_or_else(|| {
            ApiError::not_found("history_not_found", "The translation record was not found.")
        })
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
        .ok_or_else(|| {
            ApiError::not_found("memory_not_found", "The translation asset was not found.")
        })
}

async fn revise_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<MemoryRevisionRequest>,
) -> Result<Json<TranslationMemoryRecord>, ApiError> {
    if request.source_text.trim().is_empty() || request.translated_text.trim().is_empty() {
        return Err(ApiError::bad_request(
            "empty_memory",
            "Enter both source text and translated text.",
        ));
    }
    if request.source_text.chars().count() > state.config.max_text_chars
        || request.translated_text.chars().count() > state.config.max_text_chars
    {
        return Err(ApiError::bad_request(
            "memory_text_too_long",
            format!(
                "Translation asset fields cannot exceed {} characters.",
                state.config.max_text_chars
            ),
        ));
    }
    if request.target_lang == "auto" {
        return Err(ApiError::bad_request(
            "target_required",
            "Select a target language.",
        ));
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
        .ok_or_else(|| {
            ApiError::not_found("memory_not_found", "The translation asset was not found.")
        })
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
        return Err(ApiError::not_found(
            "memory_not_found",
            "The translation asset was not found.",
        ));
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
        Err(ApiError::not_found(
            "memory_not_found",
            "The translation asset was not found.",
        ))
    }
}

async fn not_found() -> impl IntoResponse {
    ApiError::not_found("route_not_found", "The requested path was not found.")
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

impl PublicConfig {
    fn new(config: &AppConfig, availability: &HashMap<String, bool>) -> Self {
        Self {
            profile: config.profile.clone(),
            display_name: config.display_name.clone(),
            default_model: config.default_model.clone(),
            max_text_chars: config.max_text_chars,
            models: config
                .models
                .iter()
                .map(|model| {
                    PublicModel::new(model, availability.get(&model.id).copied().unwrap_or(false))
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct PublicModel {
    id: String,
    label: String,
    description: String,
    privacy: String,
    available: bool,
    supported_languages: Vec<&'static str>,
}

impl PublicModel {
    fn new(model: &ModelConfig, available: bool) -> Self {
        Self {
            id: model.id.clone(),
            label: model.label.clone(),
            description: model.description.clone(),
            privacy: engine::privacy_label(model.privacy).into(),
            available,
            supported_languages: engine::supported_language_codes(model.family).to_vec(),
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
    history_error: Option<&'static str>,
    qa_warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CancelTranslationRequest {
    client_id: String,
    request_seq: u64,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    favorites: Option<bool>,
    approved: Option<bool>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct HistorySearchRequest {
    query: Option<String>,
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
    code: &'static str,
    message: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code,
            message: message.into(),
        }
    }

    fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code,
            message: message.into(),
        }
    }

    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message: message.into(),
        }
    }

    fn unavailable(error: impl std::fmt::Display) -> Self {
        eprintln!("translation engine error: {error}");
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "engine_unavailable",
            message: "The local translation engine could not complete the request.".into(),
        }
    }

    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        eprintln!("internal error: {error}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "storage_error",
            message: "The encrypted local vault could not complete the operation.".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: ErrorMessage {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        apply_security_headers(&mut response);
        response
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use reqwest::header::{AUTHORIZATION, COOKIE, HOST, ORIGIN};

    use super::*;
    use crate::{
        config::{ModelFamily, PrivacyBoundary},
        crypto::VaultCrypto,
    };

    fn test_config() -> AppConfig {
        AppConfig {
            profile: "test".into(),
            display_name: "Test".into(),
            bind: "127.0.0.1:0".into(),
            default_model: "mock".into(),
            auto_open: false,
            max_text_chars: 1_000,
            local_engine: None,
            models: vec![ModelConfig {
                id: "mock".into(),
                label: "Mock".into(),
                description: "Test model".into(),
                endpoint: "mock://local".into(),
                api_model: "mock".into(),
                family: ModelFamily::Mock,
                privacy: PrivacyBoundary::Device,
                temperature: 0.0,
                top_p: 1.0,
                top_k: 20,
                repeat_penalty: 1.0,
                context_tokens: 4_096,
                max_output_tokens: 512,
                api_key_env: None,
                runtime_api_key: None,
            }],
        }
    }

    #[tokio::test]
    async fn api_requires_the_exact_local_session_host_and_origin() {
        let temporary = tempfile::tempdir().unwrap();
        let crypto = VaultCrypto::from_key(&[7; 32]).unwrap();
        let storage = StorageWorker::start(&temporary.path().join("history.db"), crypto).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let host = address.to_string();
        let origin = format!("http://{host}");
        let config = Arc::new(test_config());
        let state = AppState {
            engines: Arc::new(EngineManager::new(config.as_ref(), reqwest::Client::new())),
            config,
            storage,
            security: Arc::new(SecurityContext::new(
                "test-session-token",
                host.as_str(),
                origin.as_str(),
            )),
        };
        let server = tokio::spawn(async move {
            axum::serve(listener, router(state)).await.unwrap();
        });
        let client = reqwest::Client::new();

        let response = client
            .get(format!("{origin}/api/health"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = client
            .get(format!("{origin}/api/health"))
            .header(COOKIE, "pt_session=test-session-token")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = client
            .get(format!("{origin}/api/health"))
            .header(AUTHORIZATION, "Bearer test-session-token")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = client
            .get(format!("{origin}/api/health"))
            .header(AUTHORIZATION, "Bearer test-session-token")
            .header(ORIGIN, "http://127.0.0.1:1")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = client
            .get(format!("{origin}/api/health"))
            .header(AUTHORIZATION, "Bearer test-session-token")
            .header(HOST, "attacker.invalid")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        server.abort();
    }

    #[tokio::test]
    async fn direct_translate_reports_engine_unavailable_for_an_unresolved_private_secret() {
        let temporary = tempfile::tempdir().unwrap();
        let crypto = VaultCrypto::from_key(&[8; 32]).unwrap();
        let storage = StorageWorker::start(&temporary.path().join("history.db"), crypto).unwrap();
        let mut config = test_config();
        config.models[0].privacy = PrivacyBoundary::PrivateNetwork;
        config.models[0].api_key_env = Some("PRIVATE_TRANSLATOR_QUALITY_API_KEY".into());
        config.models[0].runtime_api_key = None;
        let config = Arc::new(config);
        let state = AppState {
            engines: Arc::new(EngineManager::new(config.as_ref(), reqwest::Client::new())),
            config,
            storage,
            security: Arc::new(SecurityContext::new(
                "test-session-token",
                "127.0.0.1:8173",
                "http://127.0.0.1:8173",
            )),
        };

        let result = translate(
            State(state),
            Json(TranslateRequest {
                text: "Hello".into(),
                source: "en".into(),
                target: "ko".into(),
                model: "mock".into(),
                mode: "test".into(),
                save_history: false,
                client_id: None,
                request_seq: None,
            }),
        )
        .await;
        let Err(error) = result else {
            panic!("an unresolved private secret must not reach translation");
        };
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code, "engine_unavailable");
    }
}
