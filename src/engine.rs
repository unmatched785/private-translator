use std::{collections::BTreeSet, sync::OnceLock};

use anyhow::{Context, Result, bail};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::config::{ModelConfig, ModelFamily, PrivacyBoundary};

#[derive(Debug, Clone, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub target: String,
    pub model: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_true")]
    pub save_history: bool,
}

#[derive(Debug, Clone)]
pub struct ChunkTranslation {
    pub translated_text: String,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct TokenCountResponse {
    input_tokens: usize,
}

pub async fn count_prompt_tokens(
    client: &Client,
    model: &ModelConfig,
    request: &TranslateRequest,
) -> usize {
    let prompt = build_prompt(request, model.family);
    if model.family == ModelFamily::Mock {
        return conservative_token_upper_bound(&prompt);
    }

    count_openai_chat_tokens(client, model, &prompt)
        .await
        .unwrap_or_else(|_| conservative_token_upper_bound(&prompt))
}

pub async fn translate_once(
    client: &Client,
    model: &ModelConfig,
    request: &TranslateRequest,
    max_tokens: usize,
) -> Result<ChunkTranslation> {
    if max_tokens < 64 {
        bail!("번역 결과를 위한 모델 컨텍스트가 부족합니다");
    }

    match model.family {
        ModelFamily::Mock => Ok(ChunkTranslation {
            translated_text: mock_translate(&request.text, &request.target),
            truncated: false,
        }),
        ModelFamily::HyMt2 | ModelFamily::Translategemma => {
            call_openai_compatible(client, model, request, max_tokens).await
        }
    }
}

pub fn privacy_label(boundary: PrivacyBoundary) -> &'static str {
    match boundary {
        PrivacyBoundary::Device => "device",
        PrivacyBoundary::PrivateNetwork => "private_network",
    }
}

async fn call_openai_compatible(
    client: &Client,
    model: &ModelConfig,
    request: &TranslateRequest,
    max_tokens: usize,
) -> Result<ChunkTranslation> {
    let endpoint = format!("{}/chat/completions", model.endpoint.trim_end_matches('/'));
    let prompt = build_prompt(request, model.family);
    let body = json!({
        "model": model.api_model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": model.temperature,
        "top_p": model.top_p,
        "top_k": model.top_k,
        "repeat_penalty": model.repeat_penalty,
        "max_tokens": max_tokens,
        "stream": false
    });

    let response = client
        .post(&endpoint)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("번역 엔진에 연결할 수 없습니다: {}", model.label))?;

    if !response.status().is_success() {
        bail!(
            "번역 엔진이 오류를 반환했습니다: {} ({})",
            model.label,
            response.status()
        );
    }

    let response: ChatResponse = response
        .json()
        .await
        .context("번역 엔진 응답 형식이 올바르지 않습니다")?;
    let choice = response
        .choices
        .into_iter()
        .next()
        .context("번역 엔진이 결과를 반환하지 않았습니다")?;
    let text = choice.message.content.trim().to_owned();
    if text.is_empty() {
        bail!("번역 엔진이 빈 결과를 반환했습니다");
    }
    Ok(ChunkTranslation {
        translated_text: text,
        truncated: choice.finish_reason.as_deref() == Some("length"),
    })
}

async fn count_openai_chat_tokens(
    client: &Client,
    model: &ModelConfig,
    prompt: &str,
) -> Result<usize> {
    let endpoint = format!(
        "{}/chat/completions/input_tokens",
        model.endpoint.trim_end_matches('/')
    );
    let response = client
        .post(&endpoint)
        .json(&json!({
            "model": model.api_model,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .send()
        .await
        .context("모델 토큰 수를 계산할 수 없습니다")?;
    if !response.status().is_success() {
        bail!("모델 토큰 계산 API를 사용할 수 없습니다");
    }
    let count: TokenCountResponse = response
        .json()
        .await
        .context("모델 토큰 계산 응답이 올바르지 않습니다")?;
    if count.input_tokens == 0 {
        bail!("모델 토큰 계산 결과가 비어 있습니다");
    }
    Ok(count.input_tokens)
}

fn conservative_token_upper_bound(prompt: &str) -> usize {
    prompt.len().saturating_add(128)
}

fn build_prompt(request: &TranslateRequest, family: ModelFamily) -> String {
    let source = language_name(&request.source);
    let target = language_name(&request.target);
    let source_clause = if request.source == "auto" {
        String::new()
    } else {
        format!(" from {source}")
    };

    match family {
        ModelFamily::HyMt2 => format!(
            "Translate the following text{source_clause} into {target}. Note that you must ONLY output the translated result without any additional explanation:\n\n{}",
            request.text
        ),
        ModelFamily::Translategemma => format!(
            "Translate the following text{source_clause} into {target}. Output only the translation and preserve formatting, numbers, URLs, and placeholders exactly.\n\n{}",
            request.text
        ),
        ModelFamily::Mock => request.text.clone(),
    }
}

fn mock_translate(text: &str, target: &str) -> String {
    let trimmed = text.trim();
    match (trimmed.to_lowercase().as_str(), target) {
        ("hello", "ko") | ("hello!", "ko") => "안녕하세요!".into(),
        ("hello world", "ko") | ("hello, world", "ko") => "안녕하세요, 세계!".into(),
        ("thank you", "ko") => "감사합니다.".into(),
        ("안녕하세요", "en") => "Hello.".into(),
        _ => format!("[로컬 데모 · {}] {trimmed}", language_name(target)),
    }
}

pub fn qa_warnings(source: &str, translated: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if translated.trim().is_empty() {
        warnings.push("번역 결과가 비어 있습니다".into());
        return warnings;
    }

    let source_numbers = extract_matches(number_regex(), source);
    let translated_numbers = extract_matches(number_regex(), translated);
    if !source_numbers.is_subset(&translated_numbers) {
        warnings.push("원문의 숫자 또는 단위가 번역문과 다를 수 있습니다".into());
    }

    let source_urls = extract_urls(source);
    let translated_urls = extract_urls(translated);
    if source_urls != translated_urls {
        warnings.push("URL이 누락되거나 변경되었을 수 있습니다".into());
    }
    warnings
}

fn extract_matches(regex: &Regex, text: &str) -> BTreeSet<String> {
    regex
        .find_iter(text)
        .map(|capture| capture.as_str().to_owned())
        .collect()
}

fn extract_urls(text: &str) -> BTreeSet<String> {
    url_regex()
        .find_iter(text)
        .map(|capture| {
            capture
                .as_str()
                .trim_end_matches(|character: char| {
                    matches!(
                        character,
                        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\'' | '"'
                    )
                })
                .to_owned()
        })
        .collect()
}

fn number_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\d+(?:[.,]\d+)?(?:%|[a-zA-Z]{1,4})?").unwrap())
}

fn url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"https?://[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]+"#).unwrap())
}

pub fn language_name(code: &str) -> &'static str {
    match code {
        "auto" => "the detected source language",
        "ko" => "Korean",
        "en" => "English",
        "ja" => "Japanese",
        "zh" => "Chinese",
        "zh-Hant" => "Traditional Chinese",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "pt" => "Portuguese",
        "it" => "Italian",
        "ru" => "Russian",
        "ar" => "Arabic",
        "tr" => "Turkish",
        "th" => "Thai",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        "ms" => "Malay",
        "tl" => "Filipino",
        "hi" => "Hindi",
        "pl" => "Polish",
        "cs" => "Czech",
        "nl" => "Dutch",
        "uk" => "Ukrainian",
        "he" => "Hebrew",
        "fa" => "Persian",
        "bn" => "Bengali",
        "ta" => "Tamil",
        "te" => "Telugu",
        "mr" => "Marathi",
        "gu" => "Gujarati",
        "ur" => "Urdu",
        "km" => "Khmer",
        "my" => "Burmese",
        "bo" => "Tibetan",
        "kk" => "Kazakh",
        "mn" => "Mongolian",
        "ug" => "Uyghur",
        "yue" => "Cantonese",
        _ => "the selected language",
    }
}

fn default_source() -> String {
    "auto".into()
}

fn default_mode() -> String {
    "balanced".into()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qa_detects_changed_numbers_and_urls() {
        let warnings = qa_warnings(
            "Open https://example.com and pay 441 USD",
            "example.org를 열고 442 USD를 지불하세요",
        );
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn qa_allows_a_month_name_to_become_an_extra_number() {
        let warnings = qa_warnings(
            "The contract starts on 1 August 2026.",
            "계약은 2026년 8월 1일에 시작됩니다.",
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn qa_allows_korean_particles_and_sentence_punctuation_after_urls() {
        let warnings = qa_warnings(
            "Open https://intranet.example.com.",
            "https://intranet.example.com에서 여세요.",
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn mock_has_a_real_smoke_translation() {
        assert_eq!(mock_translate("Hello", "ko"), "안녕하세요!");
    }
}
