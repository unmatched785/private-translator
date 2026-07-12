use std::{collections::VecDeque, error::Error, fmt, time::Instant};

use anyhow::{Context, Result, bail};

use crate::{
    backend::TranslationBackend,
    config::{ModelConfig, ModelFamily},
    engine::{self, TranslateRequest},
};

const CONTEXT_SAFETY_TOKENS: usize = 128;
const MAX_TRANSLATION_CHUNKS: usize = 256;

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub translated_text: String,
    pub latency_ms: u64,
    pub qa_warnings: Vec<String>,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextChunk {
    text: String,
    separator_after: String,
}

#[derive(Debug)]
pub struct SupersededRequest;

impl fmt::Display for SupersededRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("A newer translation request replaced this one")
    }
}

impl Error for SupersededRequest {}

pub async fn run(
    backend: &dyn TranslationBackend,
    model: &ModelConfig,
    request: &TranslateRequest,
    is_current: &(dyn Fn() -> bool + Send + Sync),
) -> Result<TranslationResult> {
    let started = Instant::now();
    let prompt_budget = prompt_budget(model)?;
    let (leading, core, trailing) = outer_whitespace(&request.text);
    let mut pending = VecDeque::from([TextChunk {
        text: core.to_owned(),
        separator_after: trailing.to_owned(),
    }]);
    let mut translated_text = leading.to_owned();
    let mut chunk_count = 0;

    while let Some(chunk) = pending.pop_front() {
        ensure_current(is_current)?;
        let mut chunk_request = request.clone();
        chunk_request.text = chunk.text.clone();
        let prompt_tokens = backend.count_prompt_tokens(model, &chunk_request).await;
        ensure_current(is_current)?;

        if model.family != ModelFamily::Mock && prompt_tokens > prompt_budget {
            push_split(&mut pending, chunk, chunk_count)?;
            continue;
        }

        let available_output = model
            .context_tokens
            .saturating_sub(prompt_tokens.saturating_add(CONTEXT_SAFETY_TOKENS));
        let max_tokens = model.max_output_tokens.min(available_output);
        let result = backend
            .translate_once(model, &chunk_request, max_tokens)
            .await?;
        ensure_current(is_current)?;

        if result.truncated {
            push_split(&mut pending, chunk, chunk_count)?;
            continue;
        }

        translated_text.push_str(result.translated_text.trim());
        translated_text.push_str(&chunk.separator_after);
        chunk_count += 1;
    }

    if chunk_count == 0 {
        bail!("Could not create a text chunk for translation");
    }
    ensure_current(is_current)?;
    let latency_ms = started.elapsed().as_millis() as u64;
    let qa_warnings = engine::qa_warnings(&request.text, &translated_text);
    Ok(TranslationResult {
        translated_text,
        latency_ms,
        qa_warnings,
        chunk_count,
    })
}

fn ensure_current(is_current: &(dyn Fn() -> bool + Send + Sync)) -> Result<()> {
    if !is_current() {
        return Err(SupersededRequest.into());
    }
    Ok(())
}

fn prompt_budget(model: &ModelConfig) -> Result<usize> {
    if model.family == ModelFamily::Mock {
        return Ok(usize::MAX);
    }
    model
        .context_tokens
        .checked_sub(model.max_output_tokens + CONTEXT_SAFETY_TOKENS)
        .filter(|budget| *budget >= 64)
        .context("Could not create a safe translation budget for the model context")
}

fn push_split(pending: &mut VecDeque<TextChunk>, chunk: TextChunk, completed: usize) -> Result<()> {
    if completed + pending.len() + 2 > MAX_TRANSLATION_CHUNKS {
        bail!(
            "Could not split the long document into at most {} chunks",
            MAX_TRANSLATION_CHUNKS
        );
    }
    let (left, right) = split_chunk(chunk)
        .context("Text that exceeds one model context could not be split further")?;
    pending.push_front(right);
    pending.push_front(left);
    Ok(())
}

fn split_chunk(chunk: TextChunk) -> Option<(TextChunk, TextChunk)> {
    let text = chunk.text.as_str();
    let indexed = text.char_indices().collect::<Vec<_>>();
    if indexed.len() < 2 {
        return None;
    }
    let midpoint = indexed[indexed.len() / 2].0;
    let window = text.len().max(4) / 4;
    let mut boundaries = Vec::new();

    let mut index = 0;
    while index < indexed.len() {
        let (start, character) = indexed[index];
        if character.is_whitespace() {
            let mut next = index + 1;
            while next < indexed.len() && indexed[next].1.is_whitespace() {
                next += 1;
            }
            let end = indexed.get(next).map_or(text.len(), |(offset, _)| *offset);
            if start > 0 && end < text.len() {
                let separator = &text[start..end];
                let previous = text[..start].chars().next_back();
                let priority = if separator.contains('\n') {
                    0
                } else if previous.is_some_and(is_sentence_ending) {
                    1
                } else {
                    2
                };
                boundaries.push((start, end, priority, start.abs_diff(midpoint)));
            }
            index = next;
            continue;
        }

        if is_sentence_ending(character) {
            let end = indexed
                .get(index + 1)
                .map_or(text.len(), |(offset, _)| *offset);
            if end > 0 && end < text.len() && !indexed[index + 1].1.is_whitespace() {
                boundaries.push((end, end, 1, end.abs_diff(midpoint)));
            }
        }
        index += 1;
    }

    let selected = [0_u8, 1]
        .into_iter()
        .find_map(|priority| {
            boundaries
                .iter()
                .filter(|(_, _, candidate, distance)| *candidate == priority && *distance <= window)
                .min_by_key(|(_, _, _, distance)| *distance)
                .copied()
        })
        .or_else(|| {
            boundaries
                .iter()
                .min_by_key(|(_, _, priority, distance)| (*distance, *priority))
                .copied()
        });

    let (split_start, split_end) = selected
        .map(|(start, end, _, _)| (start, end))
        .unwrap_or((midpoint, midpoint));
    if split_start == 0 || split_end >= text.len() {
        return None;
    }

    let left_text = text[..split_start].to_owned();
    let internal_separator = text[split_start..split_end].to_owned();
    let right_text = text[split_end..].to_owned();
    if left_text.is_empty() || right_text.is_empty() {
        return None;
    }

    Some((
        TextChunk {
            text: left_text,
            separator_after: internal_separator,
        },
        TextChunk {
            text: right_text,
            separator_after: chunk.separator_after,
        },
    ))
}

fn is_sentence_ending(character: char) -> bool {
    matches!(character, '.' | '!' | '?' | '。' | '！' | '？')
}

fn outer_whitespace(text: &str) -> (&str, &str, &str) {
    let start = text
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map_or(text.len(), |(offset, _)| offset);
    let end = text
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map_or(start, |(offset, character)| offset + character.len_utf8());
    (&text[..start], &text[start..end], &text[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reconstruct(left: &TextChunk, right: &TextChunk) -> String {
        format!(
            "{}{}{}{}",
            left.text, left.separator_after, right.text, right.separator_after
        )
    }

    #[test]
    fn split_prefers_a_nearby_paragraph_boundary() {
        let original = "First paragraph is here.\n\nSecond paragraph is here.";
        let (left, right) = split_chunk(TextChunk {
            text: original.into(),
            separator_after: String::new(),
        })
        .unwrap();
        assert_eq!(left.separator_after, "\n\n");
        assert_eq!(reconstruct(&left, &right), original);
    }

    #[test]
    fn hard_split_is_unicode_safe_and_lossless() {
        let original = "가나다라마바사아자차카타파하";
        let (left, right) = split_chunk(TextChunk {
            text: original.into(),
            separator_after: "\n".into(),
        })
        .unwrap();
        assert!(!left.text.is_empty());
        assert!(!right.text.is_empty());
        assert_eq!(reconstruct(&left, &right), format!("{original}\n"));
    }

    #[test]
    fn outer_whitespace_is_preserved_separately() {
        assert_eq!(outer_whitespace(" \nhello\n "), (" \n", "hello", "\n "));
    }
}
