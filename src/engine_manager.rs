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
use tokio::sync::Semaphore;

use crate::{
    backend::{MockBackend, OpenAiCompatibleBackend, TranslationBackend},
    config::{AppConfig, ModelConfig, ModelFamily},
    engine::TranslateRequest,
    pipeline::{self, SupersededRequest, TranslationResult},
};

const LATEST_REQUEST_TTL: Duration = Duration::from_secs(60 * 60);
const LATEST_REQUEST_PRUNE_THRESHOLD: usize = 512;

pub struct EngineManager {
    models: HashMap<String, Arc<ManagedModel>>,
    latest: Arc<LatestRequests>,
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
            Arc::new(OpenAiCompatibleBackend::new(client));
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
        }
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
        let identity = RequestIdentity::from_request(request);
        if !self.latest.register(&identity) {
            return Err(TranslateError::Superseded);
        }

        let queued = CounterGuard::new(&model.queued);
        let permit = model
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| TranslateError::Failed(anyhow!("The translation queue stopped")));
        drop(queued);
        let _permit = permit?;

        if !self.latest.is_current(&identity) {
            return Err(TranslateError::Superseded);
        }

        let _active = CounterGuard::new(&model.active);
        let latest = Arc::clone(&self.latest);
        let is_current = move || latest.is_current(&identity);
        match pipeline::run(model.backend.as_ref(), &model.config, request, &is_current).await {
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

    #[cfg(test)]
    fn with_backend(model: ModelConfig, backend: Arc<dyn TranslationBackend>) -> Self {
        Self {
            models: HashMap::from([(
                model.id.clone(),
                Arc::new(ManagedModel::new(model, backend)),
            )]),
            latest: Arc::new(LatestRequests::default()),
        }
    }
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
}

#[derive(Default)]
struct LatestRequests {
    entries: Mutex<HashMap<String, LatestRequest>>,
}

struct LatestRequest {
    sequence: u64,
    touched_at: Instant,
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
        entries.insert(
            client_id.clone(),
            LatestRequest {
                sequence: *sequence,
                touched_at: Instant::now(),
            },
        );
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
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use anyhow::Result;

    use super::*;
    use crate::{backend::BackendFuture, config::PrivacyBoundary, engine::ChunkTranslation};

    struct SlowBackend {
        active: AtomicUsize,
        max_active: AtomicUsize,
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
                self.max_active.fetch_max(active, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.active.fetch_sub(1, Ordering::SeqCst);
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
        assert_eq!(second.await.unwrap().unwrap().translated_text, "REQUEST 2");
        assert_eq!(backend.max_active.load(Ordering::SeqCst), 1);
        assert_eq!(manager.status().active, 0);
        assert_eq!(manager.status().queued, 0);
    }
}
