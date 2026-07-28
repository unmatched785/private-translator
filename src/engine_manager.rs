use std::{
    collections::HashMap,
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::anyhow;
use reqwest::Client;
use serde::Serialize;
use tokio::sync::{Semaphore, watch};

use crate::{
    backend::{MockBackend, OpenAiCompatibleBackend, TranslationBackend},
    config::{AppConfig, ModelConfig, ModelFamily, PrivacyBoundary},
    engine::TranslateRequest,
    pipeline::{self, SupersededRequest, TranslationResult},
};

const LATEST_REQUEST_TTL: Duration = Duration::from_secs(60 * 60);
const LATEST_REQUEST_PRUNE_THRESHOLD: usize = 512;

pub struct EngineManager {
    models: HashMap<String, Arc<ManagedModel>>,
    latest: Arc<LatestRequests>,
    client: Client,
}

struct ManagedModel {
    config: ModelConfig,
    backend: Arc<dyn TranslationBackend>,
    permits: Arc<Semaphore>,
    active: AtomicUsize,
    queued: AtomicUsize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineQueueStatus {
    pub active: usize,
    pub queued: usize,
    pub models: Vec<ModelQueueStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelQueueStatus {
    pub model_id: String,
    pub active: usize,
    pub queued: usize,
}

#[derive(Debug)]
pub enum TranslateError {
    UnknownModel,
    Superseded,
    Failed(anyhow::Error),
}

impl fmt::Display for TranslateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownModel => formatter.write_str("The selected model was not found"),
            Self::Superseded => {
                formatter.write_str("A newer translation request replaced this one")
            }
            Self::Failed(error) => error.fmt(formatter),
        }
    }
}

impl Error for TranslateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl EngineManager {
    pub fn new(config: &AppConfig, client: Client) -> Self {
        let http_backend: Arc<dyn TranslationBackend> =
            Arc::new(OpenAiCompatibleBackend::new(client.clone()));
        let mock_backend: Arc<dyn TranslationBackend> = Arc::new(MockBackend);
        let models = config
            .models
            .iter()
            .cloned()
            .map(|model| {
                let backend = if model.family == ModelFamily::Mock {
                    Arc::clone(&mock_backend)
                } else {
                    Arc::clone(&http_backend)
                };
                let id = model.id.clone();
                (id, Arc::new(ManagedModel::new(model, backend)))
            })
            .collect();
        Self {
            models,
            latest: Arc::new(LatestRequests::default()),
            client,
        }
    }

    pub async fn availability(&self) -> HashMap<String, bool> {
        let mut availability = HashMap::with_capacity(self.models.len());
        let mut models = self.models.values().collect::<Vec<_>>();
        models.sort_by(|left, right| left.config.id.cmp(&right.config.id));
        for model in models {
            let available = if !has_required_runtime_auth(&model.config) {
                false
            } else if model.config.family == ModelFamily::Mock {
                true
            } else {
                probe_configured_model(&self.client, &model.config).await
            };
            availability.insert(model.config.id.clone(), available);
        }
        availability
    }

    pub async fn translate(
        &self,
        request: &TranslateRequest,
    ) -> Result<TranslationResult, TranslateError> {
        let model = self
            .models
            .get(&request.model)
            .cloned()
            .ok_or(TranslateError::UnknownModel)?;
        if !has_required_runtime_auth(&model.config) {
            return Err(TranslateError::Failed(anyhow!(
                "The private-network model authentication secret is not configured"
            )));
        }
        let identity = RequestIdentity::from_request(request);
        if !self.latest.register(&identity) {
            return Err(TranslateError::Superseded);
        }
        let mut cancellation = self.latest.subscribe(&identity);

        let queued = CounterGuard::new(&model.queued);
        let acquire = model.permits.clone().acquire_owned();
        let permit = if let Some(receiver) = cancellation.as_mut() {
            tokio::select! {
                biased;
                _ = wait_until_superseded(receiver, identity.sequence().unwrap()) => {
                    return Err(TranslateError::Superseded);
                }
                permit = acquire => permit,
            }
        } else {
            acquire.await
        }
        .map_err(|_| TranslateError::Failed(anyhow!("The translation queue stopped")));
        drop(queued);
        let _permit = permit?;

        if !self.latest.is_current(&identity) {
            return Err(TranslateError::Superseded);
        }

        let _active = CounterGuard::new(&model.active);
        let latest = Arc::clone(&self.latest);
        let is_current = move || latest.is_current(&identity);
        let pipeline = pipeline::run(model.backend.as_ref(), &model.config, request, &is_current);
        let result = if let Some(receiver) = cancellation.as_mut() {
            tokio::select! {
                biased;
                _ = wait_until_superseded(receiver, request.request_seq.unwrap()) => {
                    return Err(TranslateError::Superseded);
                }
                result = pipeline => result,
            }
        } else {
            pipeline.await
        };
        match result {
            Ok(result) => Ok(result),
            Err(error) if error.downcast_ref::<SupersededRequest>().is_some() => {
                Err(TranslateError::Superseded)
            }
            Err(error) => Err(TranslateError::Failed(error)),
        }
    }

    pub fn status(&self) -> EngineQueueStatus {
        let mut models = self
            .models
            .values()
            .map(|model| ModelQueueStatus {
                model_id: model.config.id.clone(),
                active: model.active.load(Ordering::SeqCst),
                queued: model.queued.load(Ordering::SeqCst),
            })
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.model_id.cmp(&right.model_id));
        EngineQueueStatus {
            active: models.iter().map(|model| model.active).sum(),
            queued: models.iter().map(|model| model.queued).sum(),
            models,
        }
    }

    pub fn cancel(&self, client_id: &str, sequence: u64) {
        self.latest
            .register(&RequestIdentity(Some((client_id.to_owned(), sequence))));
    }

    pub fn is_current(&self, request: &TranslateRequest) -> bool {
        self.latest
            .is_current(&RequestIdentity::from_request(request))
    }

    #[cfg(test)]
    fn with_backend(model: ModelConfig, backend: Arc<dyn TranslationBackend>) -> Self {
        Self {
            models: HashMap::from([(
                model.id.clone(),
                Arc::new(ManagedModel::new(model, backend)),
            )]),
            latest: Arc::new(LatestRequests::default()),
            client: Client::new(),
        }
    }
}

fn has_required_runtime_auth(model: &ModelConfig) -> bool {
    model.privacy != PrivacyBoundary::PrivateNetwork
        || model
            .runtime_api_key
            .as_deref()
            .is_some_and(|secret| !secret.trim().is_empty())
}

async fn probe_configured_model(client: &Client, model: &ModelConfig) -> bool {
    let endpoint = format!("{}/models", model.endpoint.trim_end_matches('/'));
    let mut request = client.get(endpoint).timeout(Duration::from_millis(700));
    if let Some(api_key) = &model.runtime_api_key {
        request = request.bearer_auth(api_key);
    }
    let Ok(Ok(response)) = tokio::time::timeout(Duration::from_secs(1), request.send()).await
    else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let Ok(body) = response.json::<serde_json::Value>().await else {
        return false;
    };
    body.get("data")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|models| {
            models.iter().any(|candidate| {
                candidate.get("id").and_then(serde_json::Value::as_str)
                    == Some(model.api_model.as_str())
            })
        })
}

impl ManagedModel {
    fn new(config: ModelConfig, backend: Arc<dyn TranslationBackend>) -> Self {
        Self {
            config,
            backend,
            permits: Arc::new(Semaphore::new(1)),
            active: AtomicUsize::new(0),
            queued: AtomicUsize::new(0),
        }
    }
}

struct CounterGuard<'a>(&'a AtomicUsize);

impl<'a> CounterGuard<'a> {
    fn new(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for CounterGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone)]
struct RequestIdentity(Option<(String, u64)>);

impl RequestIdentity {
    fn from_request(request: &TranslateRequest) -> Self {
        Self(
            request
                .client_id
                .as_ref()
                .zip(request.request_seq)
                .map(|(client_id, sequence)| (client_id.clone(), sequence)),
        )
    }

    fn sequence(&self) -> Option<u64> {
        self.0.as_ref().map(|(_, sequence)| *sequence)
    }
}

#[derive(Default)]
struct LatestRequests {
    entries: Mutex<HashMap<String, LatestRequest>>,
}

struct LatestRequest {
    sequence: u64,
    touched_at: Instant,
    signal: watch::Sender<u64>,
}

impl LatestRequests {
    fn register(&self, identity: &RequestIdentity) -> bool {
        let Some((client_id, sequence)) = &identity.0 else {
            return true;
        };
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if entries.len() >= LATEST_REQUEST_PRUNE_THRESHOLD {
            entries.retain(|_, request| request.touched_at.elapsed() < LATEST_REQUEST_TTL);
        }
        if entries
            .get(client_id)
            .is_some_and(|current| *sequence <= current.sequence)
        {
            return false;
        }
        if let Some(current) = entries.get_mut(client_id) {
            current.sequence = *sequence;
            current.touched_at = Instant::now();
            current.signal.send_replace(*sequence);
        } else {
            let (signal, _) = watch::channel(*sequence);
            entries.insert(
                client_id.clone(),
                LatestRequest {
                    sequence: *sequence,
                    touched_at: Instant::now(),
                    signal,
                },
            );
        }
        true
    }

    fn is_current(&self, identity: &RequestIdentity) -> bool {
        let Some((client_id, sequence)) = &identity.0 else {
            return true;
        };
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(client_id)
            .is_some_and(|current| current.sequence == *sequence)
    }

    fn subscribe(&self, identity: &RequestIdentity) -> Option<watch::Receiver<u64>> {
        let (client_id, _) = identity.0.as_ref()?;
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(client_id)
            .map(|current| current.signal.subscribe())
    }
}

async fn wait_until_superseded(receiver: &mut watch::Receiver<u64>, sequence: u64) {
    loop {
        if *receiver.borrow() != sequence {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use anyhow::Result;
    use axum::{Json, Router, http::HeaderMap, routing::get};

    use super::*;
    use crate::{backend::BackendFuture, config::PrivacyBoundary, engine::ChunkTranslation};

    struct SlowBackend {
        active: AtomicUsize,
        max_active: AtomicUsize,
    }

    struct ActiveCall<'a>(&'a AtomicUsize);

    impl Drop for ActiveCall<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl TranslationBackend for SlowBackend {
        fn count_prompt_tokens<'a>(
            &'a self,
            _model: &'a ModelConfig,
            _request: &'a TranslateRequest,
        ) -> BackendFuture<'a, usize> {
            Box::pin(async { 1 })
        }

        fn translate_once<'a>(
            &'a self,
            _model: &'a ModelConfig,
            request: &'a TranslateRequest,
            _max_tokens: usize,
        ) -> BackendFuture<'a, Result<ChunkTranslation>> {
            Box::pin(async move {
                let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                let _active = ActiveCall(&self.active);
                self.max_active.fetch_max(active, Ordering::SeqCst);
                let delay = if request.text.ends_with('1') {
                    Duration::from_millis(500)
                } else {
                    Duration::from_millis(20)
                };
                tokio::time::sleep(delay).await;
                Ok(ChunkTranslation {
                    translated_text: request.text.to_uppercase(),
                    truncated: false,
                })
            })
        }
    }

    fn model() -> ModelConfig {
        ModelConfig {
            id: "slow".into(),
            label: "Slow".into(),
            description: "test".into(),
            endpoint: "http://127.0.0.1:9999/v1".into(),
            api_model: "slow".into(),
            family: ModelFamily::Mock,
            privacy: PrivacyBoundary::Device,
            temperature: 0.0,
            top_p: 1.0,
            top_k: 20,
            repeat_penalty: 1.0,
            context_tokens: 4096,
            max_output_tokens: 512,
            api_key_env: None,
            runtime_api_key: None,
        }
    }

    fn request(sequence: u64) -> TranslateRequest {
        TranslateRequest {
            text: format!("request {sequence}"),
            source: "en".into(),
            target: "ko".into(),
            model: "slow".into(),
            mode: "test".into(),
            save_history: false,
            client_id: Some("browser-tab".into()),
            request_seq: Some(sequence),
        }
    }

    #[tokio::test]
    async fn one_model_is_serialized_and_only_latest_request_completes() {
        let backend = Arc::new(SlowBackend {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        });
        let manager = Arc::new(EngineManager::with_backend(model(), backend.clone()));

        let first_manager = Arc::clone(&manager);
        let first = tokio::spawn(async move { first_manager.translate(&request(1)).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        let second_manager = Arc::clone(&manager);
        let second = tokio::spawn(async move { second_manager.translate(&request(2)).await });

        assert!(matches!(
            first.await.unwrap(),
            Err(TranslateError::Superseded)
        ));
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(150), second)
                .await
                .expect("the replacement request must not wait for old inference")
                .unwrap()
                .unwrap()
                .translated_text,
            "REQUEST 2"
        );
        assert_eq!(backend.max_active.load(Ordering::SeqCst), 1);
        assert_eq!(manager.status().active, 0);
        assert_eq!(manager.status().queued, 0);
    }

    #[tokio::test]
    async fn explicit_cancel_interrupts_in_flight_inference() {
        let backend = Arc::new(SlowBackend {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        });
        let manager = Arc::new(EngineManager::with_backend(model(), backend.clone()));

        let first_manager = Arc::clone(&manager);
        let first = tokio::spawn(async move { first_manager.translate(&request(1)).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        manager.cancel("browser-tab", 2);

        assert!(matches!(
            tokio::time::timeout(Duration::from_millis(150), first)
                .await
                .expect("cancel must interrupt active inference")
                .unwrap(),
            Err(TranslateError::Superseded)
        ));
        assert_eq!(backend.active.load(Ordering::SeqCst), 0);
        assert_eq!(manager.status().active, 0);
        assert_eq!(manager.status().queued, 0);
    }

    #[tokio::test]
    async fn availability_probes_real_models_with_bearer_auth_and_identity() {
        async fn models(headers: HeaderMap) -> Json<serde_json::Value> {
            assert_eq!(
                headers.get("authorization").unwrap(),
                "Bearer readiness-secret"
            );
            Json(serde_json::json!({ "data": [{ "id": "ready-model" }] }))
        }

        let app = Router::new().route("/v1/models", get(models));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut config = model();
        config.family = ModelFamily::HyMt2;
        config.endpoint = format!("http://{address}/v1");
        config.api_model = "ready-model".into();
        config.runtime_api_key = Some("readiness-secret".into());
        let client = Client::new();

        assert!(probe_configured_model(&client, &config).await);
        config.api_model = "wrong-model".into();
        assert!(!probe_configured_model(&client, &config).await);
        server.abort();
    }

    #[tokio::test]
    async fn private_network_model_without_secret_is_blocked_before_its_backend() {
        let backend = Arc::new(SlowBackend {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        });
        let mut config = model();
        config.privacy = PrivacyBoundary::PrivateNetwork;
        config.api_key_env = Some("PRIVATE_TRANSLATOR_QUALITY_API_KEY".into());
        config.runtime_api_key = None;
        let manager = EngineManager::with_backend(config, backend.clone());

        assert_eq!(manager.availability().await.get("slow"), Some(&false));
        assert!(matches!(
            manager.translate(&request(1)).await,
            Err(TranslateError::Failed(_))
        ));
        assert_eq!(backend.max_active.load(Ordering::SeqCst), 0);
        assert_eq!(manager.status().active, 0);
        assert_eq!(manager.status().queued, 0);
    }
}
