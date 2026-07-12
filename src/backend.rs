use std::{future::Future, pin::Pin};

use anyhow::Result;
use reqwest::Client;

use crate::{
    config::ModelConfig,
    engine::{self, ChunkTranslation, TranslateRequest},
};

pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait TranslationBackend: Send + Sync {
    fn count_prompt_tokens<'a>(
        &'a self,
        model: &'a ModelConfig,
        request: &'a TranslateRequest,
    ) -> BackendFuture<'a, usize>;

    fn translate_once<'a>(
        &'a self,
        model: &'a ModelConfig,
        request: &'a TranslateRequest,
        max_tokens: usize,
    ) -> BackendFuture<'a, Result<ChunkTranslation>>;
}

pub struct OpenAiCompatibleBackend {
    client: Client,
}

impl OpenAiCompatibleBackend {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl TranslationBackend for OpenAiCompatibleBackend {
    fn count_prompt_tokens<'a>(
        &'a self,
        model: &'a ModelConfig,
        request: &'a TranslateRequest,
    ) -> BackendFuture<'a, usize> {
        Box::pin(engine::count_prompt_tokens(&self.client, model, request))
    }

    fn translate_once<'a>(
        &'a self,
        model: &'a ModelConfig,
        request: &'a TranslateRequest,
        max_tokens: usize,
    ) -> BackendFuture<'a, Result<ChunkTranslation>> {
        Box::pin(engine::translate_once(
            &self.client,
            model,
            request,
            max_tokens,
        ))
    }
}

pub struct MockBackend;

impl TranslationBackend for MockBackend {
    fn count_prompt_tokens<'a>(
        &'a self,
        _model: &'a ModelConfig,
        _request: &'a TranslateRequest,
    ) -> BackendFuture<'a, usize> {
        Box::pin(async { 0 })
    }

    fn translate_once<'a>(
        &'a self,
        _model: &'a ModelConfig,
        request: &'a TranslateRequest,
        _max_tokens: usize,
    ) -> BackendFuture<'a, Result<ChunkTranslation>> {
        Box::pin(async move {
            Ok(ChunkTranslation {
                translated_text: engine::mock_translate(&request.text, &request.target),
                truncated: false,
            })
        })
    }
}
