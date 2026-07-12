use std::{path::Path, sync::Mutex};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::VaultCrypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: String,
    pub created_at: i64,
    pub favorite: bool,
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub model_id: String,
    pub model_label: String,
    pub mode: String,
    pub privacy: String,
    pub latency_ms: u64,
    pub qa_warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NewHistoryRecord {
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub model_id: String,
    pub model_label: String,
    pub mode: String,
    pub privacy: String,
    pub latency_ms: u64,
    pub qa_warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub query: Option<String>,
    pub favorites_only: bool,
    pub limit: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedPayload {
    source_text: String,
    translated_text: String,
    source_lang: String,
    target_lang: String,
    model_id: String,
    model_label: String,
    mode: String,
    privacy: String,
    latency_ms: u64,
    qa_warnings: Vec<String>,
}

pub struct HistoryStore {
    connection: Mutex<Connection>,
    crypto: VaultCrypto,
}

impl HistoryStore {
    pub fn open(path: &Path, crypto: VaultCrypto) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("기록 DB 폴더를 만들 수 없습니다")?;
        }
        let connection = Connection::open(path).context("로컬 기록 DB를 열 수 없습니다")?;
        connection
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = FULL;
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS history (
                    id TEXT PRIMARY KEY,
                    created_at INTEGER NOT NULL,
                    favorite INTEGER NOT NULL DEFAULT 0,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_history_created_at
                    ON history(created_at DESC);
                ",
            )
            .context("기록 DB를 초기화할 수 없습니다")?;

        Ok(Self {
            connection: Mutex::new(connection),
            crypto,
        })
    }

    pub fn insert(&self, new_record: NewHistoryRecord) -> Result<HistoryRecord> {
        let id = Uuid::new_v4().to_string();
        let created_at = now_unix_ms();
        let payload = EncryptedPayload {
            source_text: new_record.source_text,
            translated_text: new_record.translated_text,
            source_lang: new_record.source_lang,
            target_lang: new_record.target_lang,
            model_id: new_record.model_id,
            model_label: new_record.model_label,
            mode: new_record.mode,
            privacy: new_record.privacy,
            latency_ms: new_record.latency_ms,
            qa_warnings: new_record.qa_warnings,
        };
        let serialized =
            serde_json::to_vec(&payload).context("번역 기록을 직렬화할 수 없습니다")?;
        let aad = aad(&id, created_at);
        let (nonce, ciphertext) = self.crypto.encrypt(&serialized, &aad)?;

        let connection = self.connection.lock().expect("history mutex poisoned");
        connection
            .execute(
                "INSERT INTO history (id, created_at, favorite, nonce, ciphertext)
                 VALUES (?1, ?2, 0, ?3, ?4)",
                params![id, created_at, nonce, ciphertext],
            )
            .context("번역 기록을 저장할 수 없습니다")?;

        Ok(HistoryRecord {
            id,
            created_at,
            favorite: false,
            source_text: payload.source_text,
            translated_text: payload.translated_text,
            source_lang: payload.source_lang,
            target_lang: payload.target_lang,
            model_id: payload.model_id,
            model_label: payload.model_label,
            mode: payload.mode,
            privacy: payload.privacy,
            latency_ms: payload.latency_ms,
            qa_warnings: payload.qa_warnings,
        })
    }

    pub fn list(&self, filter: HistoryFilter) -> Result<Vec<HistoryRecord>> {
        let scan_limit = if filter
            .query
            .as_deref()
            .is_some_and(|q| !q.trim().is_empty())
        {
            10_000
        } else {
            filter.limit.clamp(1, 500)
        };
        let rows = self.read_rows(scan_limit, filter.favorites_only)?;
        let query = filter
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(|query| query.to_lowercase());
        let limit = filter.limit.clamp(1, 500);

        let mut records = Vec::new();
        for row in rows {
            let record = self.decrypt_row(row)?;
            if let Some(query) = &query {
                let haystack = format!(
                    "{}\n{}\n{}\n{}",
                    record.source_text,
                    record.translated_text,
                    record.source_lang,
                    record.target_lang
                )
                .to_lowercase();
                if !haystack.contains(query) {
                    continue;
                }
            }
            records.push(record);
            if records.len() >= limit {
                break;
            }
        }
        Ok(records)
    }

    pub fn get(&self, id: &str) -> Result<Option<HistoryRecord>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let row = connection
            .query_row(
                "SELECT id, created_at, favorite, nonce, ciphertext FROM history WHERE id = ?1",
                [id],
                |row| {
                    Ok(EncryptedRow {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        favorite: row.get::<_, i64>(2)? != 0,
                        nonce: row.get(3)?,
                        ciphertext: row.get(4)?,
                    })
                },
            )
            .optional()
            .context("번역 기록을 읽을 수 없습니다")?;
        drop(connection);
        row.map(|row| self.decrypt_row(row)).transpose()
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<bool> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let changed = connection
            .execute(
                "UPDATE history SET favorite = ?2 WHERE id = ?1",
                params![id, if favorite { 1_i64 } else { 0_i64 }],
            )
            .context("즐겨찾기를 변경할 수 없습니다")?;
        Ok(changed > 0)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let changed = connection
            .execute("DELETE FROM history WHERE id = ?1", [id])
            .context("번역 기록을 삭제할 수 없습니다")?;
        Ok(changed > 0)
    }

    pub fn count(&self) -> Result<u64> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .context("번역 기록 수를 읽을 수 없습니다")?;
        Ok(count.max(0) as u64)
    }

    fn read_rows(&self, limit: usize, favorites_only: bool) -> Result<Vec<EncryptedRow>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let sql = if favorites_only {
            "SELECT id, created_at, favorite, nonce, ciphertext
             FROM history WHERE favorite = 1 ORDER BY created_at DESC LIMIT ?1"
        } else {
            "SELECT id, created_at, favorite, nonce, ciphertext
             FROM history ORDER BY created_at DESC LIMIT ?1"
        };
        let mut statement = connection
            .prepare(sql)
            .context("기록 조회를 준비할 수 없습니다")?;
        let mapped = statement
            .query_map([limit as i64], |row| {
                Ok(EncryptedRow {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    favorite: row.get::<_, i64>(2)? != 0,
                    nonce: row.get(3)?,
                    ciphertext: row.get(4)?,
                })
            })
            .context("기록을 조회할 수 없습니다")?;
        mapped
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("기록 행을 읽을 수 없습니다")
    }

    fn decrypt_row(&self, row: EncryptedRow) -> Result<HistoryRecord> {
        let aad = aad(&row.id, row.created_at);
        let plain = self.crypto.decrypt(&row.nonce, &row.ciphertext, &aad)?;
        let payload: EncryptedPayload =
            serde_json::from_slice(&plain).context("암호화된 기록 내용이 손상되었습니다")?;
        Ok(HistoryRecord {
            id: row.id,
            created_at: row.created_at,
            favorite: row.favorite,
            source_text: payload.source_text,
            translated_text: payload.translated_text,
            source_lang: payload.source_lang,
            target_lang: payload.target_lang,
            model_id: payload.model_id,
            model_label: payload.model_label,
            mode: payload.mode,
            privacy: payload.privacy,
            latency_ms: payload.latency_ms,
            qa_warnings: payload.qa_warnings,
        })
    }
}

struct EncryptedRow {
    id: String,
    created_at: i64,
    favorite: bool,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

fn aad(id: &str, created_at: i64) -> Vec<u8> {
    format!("private-translator:v1:{id}:{created_at}").into_bytes()
}

fn now_unix_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NewHistoryRecord {
        NewHistoryRecord {
            source_text: "Private contract 441".into(),
            translated_text: "비공개 계약서 441".into(),
            source_lang: "en".into(),
            target_lang: "ko".into(),
            model_id: "test-model".into(),
            model_label: "Test Model".into(),
            mode: "balanced".into(),
            privacy: "device".into(),
            latency_ms: 42,
            qa_warnings: vec![],
        }
    }

    #[test]
    fn history_is_encrypted_searchable_and_mutable() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let crypto = VaultCrypto::from_key(&[9_u8; 32]).unwrap();
        let store = HistoryStore::open(&database, crypto).unwrap();

        let inserted = store.insert(sample()).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(
            store.get(&inserted.id).unwrap().unwrap().translated_text,
            "비공개 계약서 441"
        );

        let found = store
            .list(HistoryFilter {
                query: Some("계약서".into()),
                favorites_only: false,
                limit: 20,
            })
            .unwrap();
        assert_eq!(found.len(), 1);

        assert!(store.set_favorite(&inserted.id, true).unwrap());
        assert!(store.get(&inserted.id).unwrap().unwrap().favorite);
        assert!(store.delete(&inserted.id).unwrap());
        assert_eq!(store.count().unwrap(), 0);

        drop(store);
        let bytes = std::fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| std::fs::read(entry.path()).ok())
            .flatten()
            .collect::<Vec<_>>();
        let disk = String::from_utf8_lossy(&bytes);
        assert!(!disk.contains("Private contract"));
        assert!(!disk.contains("비공개 계약서"));
    }
}
