use std::{path::Path, sync::Mutex};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::VaultCrypto;

const SCHEMA_VERSION: i64 = 2;

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
    #[serde(default)]
    pub approved_memory_id: Option<String>,
    #[serde(default)]
    pub approved_revision: Option<u32>,
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
    pub approved_only: bool,
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
    pub(crate) connection: Mutex<Connection>,
    pub(crate) crypto: VaultCrypto,
}

impl HistoryStore {
    pub fn open(path: &Path, crypto: VaultCrypto) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Could not create the vault database directory")?;
        }
        let mut connection =
            Connection::open(path).context("Could not open the local vault database")?;
        connection
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = FULL;
                PRAGMA foreign_keys = ON;
                ",
            )
            .context("Could not apply vault database safety settings")?;
        migrate(&mut connection)?;

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
            serde_json::to_vec(&payload).context("Could not serialize the translation record")?;
        let aad = aad(&id, created_at);
        let (nonce, ciphertext) = self.crypto.encrypt(&serialized, &aad)?;

        let connection = self.connection.lock().expect("history mutex poisoned");
        connection
            .execute(
                "INSERT INTO history (id, created_at, favorite, nonce, ciphertext)
                 VALUES (?1, ?2, 0, ?3, ?4)",
                params![id, created_at, nonce, ciphertext],
            )
            .context("Could not save the translation record")?;

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
            approved_memory_id: None,
            approved_revision: None,
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
        let rows = self.read_rows(scan_limit, filter.favorites_only, filter.approved_only)?;
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
                "SELECT h.id, h.created_at, h.favorite, h.nonce, h.ciphertext,
                        tm.id, tm.current_revision
                 FROM history h
                 LEFT JOIN translation_memory tm ON tm.history_id = h.id
                 WHERE h.id = ?1",
                [id],
                |row| {
                    Ok(EncryptedRow {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        favorite: row.get::<_, i64>(2)? != 0,
                        nonce: row.get(3)?,
                        ciphertext: row.get(4)?,
                        approved_memory_id: row.get(5)?,
                        approved_revision: row.get::<_, Option<i64>>(6)?.map(|value| value as u32),
                    })
                },
            )
            .optional()
            .context("Could not read the translation record")?;
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
            .context("Could not change the favorite state")?;
        Ok(changed > 0)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let changed = connection
            .execute("DELETE FROM history WHERE id = ?1", [id])
            .context("Could not delete the translation record")?;
        Ok(changed > 0)
    }

    pub fn count(&self) -> Result<u64> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .context("Could not count translation records")?;
        Ok(count.max(0) as u64)
    }

    fn read_rows(
        &self,
        limit: usize,
        favorites_only: bool,
        approved_only: bool,
    ) -> Result<Vec<EncryptedRow>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let sql = match (favorites_only, approved_only) {
            (true, true) => {
                "SELECT h.id, h.created_at, h.favorite, h.nonce, h.ciphertext,
                        tm.id, tm.current_revision
                 FROM history h
                 LEFT JOIN translation_memory tm ON tm.history_id = h.id
                 WHERE h.favorite = 1 AND tm.id IS NOT NULL
                 ORDER BY h.created_at DESC LIMIT ?1"
            }
            (true, false) => {
                "SELECT h.id, h.created_at, h.favorite, h.nonce, h.ciphertext,
                        tm.id, tm.current_revision
                 FROM history h
                 LEFT JOIN translation_memory tm ON tm.history_id = h.id
                 WHERE h.favorite = 1 ORDER BY h.created_at DESC LIMIT ?1"
            }
            (false, true) => {
                "SELECT h.id, h.created_at, h.favorite, h.nonce, h.ciphertext,
                        tm.id, tm.current_revision
                 FROM history h
                 LEFT JOIN translation_memory tm ON tm.history_id = h.id
                 WHERE tm.id IS NOT NULL ORDER BY h.created_at DESC LIMIT ?1"
            }
            (false, false) => {
                "SELECT h.id, h.created_at, h.favorite, h.nonce, h.ciphertext,
                        tm.id, tm.current_revision
                 FROM history h
                 LEFT JOIN translation_memory tm ON tm.history_id = h.id
                 ORDER BY h.created_at DESC LIMIT ?1"
            }
        };
        let mut statement = connection
            .prepare(sql)
            .context("Could not prepare the history query")?;
        let mapped = statement
            .query_map([limit as i64], |row| {
                Ok(EncryptedRow {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    favorite: row.get::<_, i64>(2)? != 0,
                    nonce: row.get(3)?,
                    ciphertext: row.get(4)?,
                    approved_memory_id: row.get(5)?,
                    approved_revision: row.get::<_, Option<i64>>(6)?.map(|value| value as u32),
                })
            })
            .context("Could not query translation history")?;
        mapped
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Could not read a history row")
    }

    fn decrypt_row(&self, row: EncryptedRow) -> Result<HistoryRecord> {
        let aad = aad(&row.id, row.created_at);
        let plain = self.crypto.decrypt(&row.nonce, &row.ciphertext, &aad)?;
        let payload: EncryptedPayload =
            serde_json::from_slice(&plain).context("The encrypted history payload is corrupted")?;
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
            approved_memory_id: row.approved_memory_id,
            approved_revision: row.approved_revision,
        })
    }
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let current_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("Could not read the vault database version")?;
    if current_version > SCHEMA_VERSION {
        bail!(
            "The vault database is newer than this application (database: {current_version}, supported: {SCHEMA_VERSION})"
        );
    }

    if current_version < 1 {
        let transaction = connection
            .transaction()
            .context("Could not begin the vault database migration")?;
        transaction
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS history (
                    id TEXT PRIMARY KEY,
                    created_at INTEGER NOT NULL,
                    favorite INTEGER NOT NULL DEFAULT 0,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_history_created_at
                    ON history(created_at DESC);
                PRAGMA user_version = 1;
                ",
            )
            .context("Vault database v1 migration failed")?;
        transaction
            .commit()
            .context("Could not commit the vault database v1 migration")?;
    }

    if current_version < 2 {
        let transaction = connection
            .transaction()
            .context("Could not begin the translation asset database migration")?;
        transaction
            .execute_batch(
                "
                CREATE TABLE translation_memory (
                    id TEXT PRIMARY KEY,
                    history_id TEXT UNIQUE,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    current_revision INTEGER NOT NULL CHECK (current_revision > 0),
                    FOREIGN KEY (history_id) REFERENCES history(id) ON DELETE SET NULL
                );
                CREATE INDEX idx_translation_memory_updated_at
                    ON translation_memory(updated_at DESC);
                CREATE TABLE translation_memory_revisions (
                    memory_id TEXT NOT NULL,
                    revision INTEGER NOT NULL CHECK (revision > 0),
                    created_at INTEGER NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    PRIMARY KEY (memory_id, revision),
                    FOREIGN KEY (memory_id) REFERENCES translation_memory(id) ON DELETE CASCADE
                );
                PRAGMA user_version = 2;
                ",
            )
            .context("Translation asset database v2 migration failed")?;
        transaction
            .commit()
            .context("Could not commit the translation asset database v2 migration")?;
    }
    Ok(())
}

struct EncryptedRow {
    id: String,
    created_at: i64,
    favorite: bool,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    approved_memory_id: Option<String>,
    approved_revision: Option<u32>,
}

fn aad(id: &str, created_at: i64) -> Vec<u8> {
    format!("private-translator:v1:{id}:{created_at}").into_bytes()
}

pub(crate) fn now_unix_ms() -> i64 {
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
                approved_only: false,
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

    #[test]
    fn legacy_database_is_migrated_without_dropping_rows() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE history (
                    id TEXT PRIMARY KEY,
                    created_at INTEGER NOT NULL,
                    favorite INTEGER NOT NULL DEFAULT 0,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL
                );
                INSERT INTO history VALUES ('legacy', 1, 0, X'00', X'00');
                ",
            )
            .unwrap();
        drop(connection);

        let crypto = VaultCrypto::from_key(&[5_u8; 32]).unwrap();
        let store = HistoryStore::open(&database, crypto).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        let version: i64 = store
            .connection
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }
}
