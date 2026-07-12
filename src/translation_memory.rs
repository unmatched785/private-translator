use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::history::{HistoryStore, now_unix_ms};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationMemoryRecord {
    pub id: String,
    pub history_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: u32,
    pub revision_created_at: i64,
    pub revision_kind: String,
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
pub struct NewMemoryRevision {
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub qa_warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedMemoryPayload {
    revision_kind: String,
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

impl HistoryStore {
    pub fn approve_history(&self, history_id: &str) -> Result<Option<TranslationMemoryRecord>> {
        if let Some(existing) = self.get_memory_by_history(history_id)? {
            return Ok(Some(existing));
        }
        let Some(history) = self.get(history_id)? else {
            return Ok(None);
        };

        let id = Uuid::new_v4().to_string();
        let created_at = now_unix_ms();
        let revision = 1;
        let payload = EncryptedMemoryPayload {
            revision_kind: "approved".into(),
            source_text: history.source_text,
            translated_text: history.translated_text,
            source_lang: history.source_lang,
            target_lang: history.target_lang,
            model_id: history.model_id,
            model_label: history.model_label,
            mode: history.mode,
            privacy: history.privacy,
            latency_ms: history.latency_ms,
            qa_warnings: history.qa_warnings,
        };
        let (nonce, ciphertext) = self.encrypt_memory(&id, revision, created_at, &payload)?;

        let mut connection = self.connection.lock().expect("history mutex poisoned");
        let transaction = connection
            .transaction()
            .context("Could not begin translation asset approval")?;
        transaction
            .execute(
                "INSERT INTO translation_memory
                    (id, history_id, created_at, updated_at, current_revision)
                 VALUES (?1, ?2, ?3, ?3, ?4)",
                params![id, history_id, created_at, revision],
            )
            .context("Could not create the approved translation asset")?;
        transaction
            .execute(
                "INSERT INTO translation_memory_revisions
                    (memory_id, revision, created_at, nonce, ciphertext)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, revision, created_at, nonce, ciphertext],
            )
            .context("Could not save the first approved translation asset revision")?;
        transaction
            .commit()
            .context("Could not commit the approved translation asset")?;

        Ok(Some(memory_record(
            id,
            Some(history_id.to_owned()),
            created_at,
            created_at,
            revision,
            created_at,
            payload,
        )))
    }

    pub fn get_memory(&self, id: &str) -> Result<Option<TranslationMemoryRecord>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let row = connection
            .query_row(
                "SELECT m.id, m.history_id, m.created_at, m.updated_at,
                        r.revision, r.created_at, r.nonce, r.ciphertext
                 FROM translation_memory m
                 JOIN translation_memory_revisions r
                   ON r.memory_id = m.id AND r.revision = m.current_revision
                 WHERE m.id = ?1",
                [id],
                read_memory_row,
            )
            .optional()
            .context("Could not read the translation asset")?;
        drop(connection);
        row.map(|row| self.decrypt_memory_row(row)).transpose()
    }

    pub fn list_memory(&self, limit: usize) -> Result<Vec<TranslationMemoryRecord>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let mut statement = connection
            .prepare(
                "SELECT m.id, m.history_id, m.created_at, m.updated_at,
                        r.revision, r.created_at, r.nonce, r.ciphertext
                 FROM translation_memory m
                 JOIN translation_memory_revisions r
                   ON r.memory_id = m.id AND r.revision = m.current_revision
                 ORDER BY m.updated_at DESC LIMIT ?1",
            )
            .context("Could not prepare the translation asset list query")?;
        let rows = statement
            .query_map([limit.clamp(1, 500) as i64], read_memory_row)
            .context("Could not query translation assets")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Could not read a translation asset row")?;
        drop(statement);
        drop(connection);
        rows.into_iter()
            .map(|row| self.decrypt_memory_row(row))
            .collect()
    }

    pub fn revise_memory(
        &self,
        id: &str,
        revision_input: NewMemoryRevision,
    ) -> Result<Option<TranslationMemoryRecord>> {
        let Some(current) = self.get_memory(id)? else {
            return Ok(None);
        };
        let revision = current
            .revision
            .checked_add(1)
            .context("The translation asset revision number is too large")?;
        let revision_created_at = now_unix_ms();
        let payload = EncryptedMemoryPayload {
            revision_kind: "edited".into(),
            source_text: revision_input.source_text,
            translated_text: revision_input.translated_text,
            source_lang: revision_input.source_lang,
            target_lang: revision_input.target_lang,
            model_id: current.model_id.clone(),
            model_label: current.model_label.clone(),
            mode: current.mode.clone(),
            privacy: current.privacy.clone(),
            latency_ms: current.latency_ms,
            qa_warnings: revision_input.qa_warnings,
        };
        let (nonce, ciphertext) =
            self.encrypt_memory(id, revision, revision_created_at, &payload)?;

        let mut connection = self.connection.lock().expect("history mutex poisoned");
        let transaction = connection
            .transaction()
            .context("Could not begin the translation asset revision")?;
        transaction
            .execute(
                "INSERT INTO translation_memory_revisions
                    (memory_id, revision, created_at, nonce, ciphertext)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, revision, revision_created_at, nonce, ciphertext],
            )
            .context("Could not save the new translation asset revision")?;
        let changed = transaction
            .execute(
                "UPDATE translation_memory
                 SET updated_at = ?2, current_revision = ?3
                 WHERE id = ?1 AND current_revision = ?4",
                params![id, revision_created_at, revision, current.revision],
            )
            .context("Could not update the current translation asset revision")?;
        if changed != 1 {
            bail!(
                "The translation asset changed concurrently, so the new revision was not applied"
            );
        }
        transaction
            .commit()
            .context("Could not commit the translation asset revision")?;

        Ok(Some(memory_record(
            current.id,
            current.history_id,
            current.created_at,
            revision_created_at,
            revision,
            revision_created_at,
            payload,
        )))
    }

    pub fn list_memory_revisions(&self, id: &str) -> Result<Vec<TranslationMemoryRecord>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let mut statement = connection
            .prepare(
                "SELECT m.id, m.history_id, m.created_at, m.updated_at,
                        r.revision, r.created_at, r.nonce, r.ciphertext
                 FROM translation_memory m
                 JOIN translation_memory_revisions r ON r.memory_id = m.id
                 WHERE m.id = ?1 ORDER BY r.revision DESC",
            )
            .context("Could not prepare the translation asset revision query")?;
        let rows = statement
            .query_map([id], read_memory_row)
            .context("Could not query translation asset revisions")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Could not read a translation asset revision row")?;
        drop(statement);
        drop(connection);
        rows.into_iter()
            .map(|row| self.decrypt_memory_row(row))
            .collect()
    }

    pub fn delete_memory(&self, id: &str) -> Result<bool> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let changed = connection
            .execute("DELETE FROM translation_memory WHERE id = ?1", [id])
            .context("Could not delete the translation asset")?;
        Ok(changed > 0)
    }

    fn get_memory_by_history(&self, history_id: &str) -> Result<Option<TranslationMemoryRecord>> {
        let connection = self.connection.lock().expect("history mutex poisoned");
        let row = connection
            .query_row(
                "SELECT m.id, m.history_id, m.created_at, m.updated_at,
                        r.revision, r.created_at, r.nonce, r.ciphertext
                 FROM translation_memory m
                 JOIN translation_memory_revisions r
                   ON r.memory_id = m.id AND r.revision = m.current_revision
                 WHERE m.history_id = ?1",
                [history_id],
                read_memory_row,
            )
            .optional()
            .context("Could not read the translation asset linked to history")?;
        drop(connection);
        row.map(|row| self.decrypt_memory_row(row)).transpose()
    }

    fn encrypt_memory(
        &self,
        id: &str,
        revision: u32,
        created_at: i64,
        payload: &EncryptedMemoryPayload,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let serialized =
            serde_json::to_vec(payload).context("Could not serialize the translation asset")?;
        self.crypto
            .encrypt(&serialized, &memory_aad(id, revision, created_at))
    }

    fn decrypt_memory_row(&self, row: EncryptedMemoryRow) -> Result<TranslationMemoryRecord> {
        let plain = self.crypto.decrypt(
            &row.nonce,
            &row.ciphertext,
            &memory_aad(&row.id, row.revision, row.revision_created_at),
        )?;
        let payload: EncryptedMemoryPayload = serde_json::from_slice(&plain)
            .context("The encrypted translation asset is corrupted")?;
        Ok(memory_record(
            row.id,
            row.history_id,
            row.created_at,
            row.updated_at,
            row.revision,
            row.revision_created_at,
            payload,
        ))
    }
}

struct EncryptedMemoryRow {
    id: String,
    history_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    revision: u32,
    revision_created_at: i64,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

fn read_memory_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncryptedMemoryRow> {
    Ok(EncryptedMemoryRow {
        id: row.get(0)?,
        history_id: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        revision: row.get::<_, i64>(4)? as u32,
        revision_created_at: row.get(5)?,
        nonce: row.get(6)?,
        ciphertext: row.get(7)?,
    })
}

fn memory_record(
    id: String,
    history_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    revision: u32,
    revision_created_at: i64,
    payload: EncryptedMemoryPayload,
) -> TranslationMemoryRecord {
    TranslationMemoryRecord {
        id,
        history_id,
        created_at,
        updated_at,
        revision,
        revision_created_at,
        revision_kind: payload.revision_kind,
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
    }
}

fn memory_aad(id: &str, revision: u32, created_at: i64) -> Vec<u8> {
    format!("private-translator:memory:v1:{id}:{revision}:{created_at}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crypto::VaultCrypto, history::NewHistoryRecord};

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
    fn approved_memory_is_separate_encrypted_and_revisioned() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let crypto = VaultCrypto::from_key(&[3_u8; 32]).unwrap();
        let store = HistoryStore::open(&database, crypto).unwrap();

        let history = store.insert(sample()).unwrap();
        let approved = store.approve_history(&history.id).unwrap().unwrap();
        assert_eq!(approved.revision, 1);
        assert_eq!(approved.revision_kind, "approved");
        let approved_again = store.approve_history(&history.id).unwrap().unwrap();
        assert_eq!(approved_again.id, approved.id);
        let linked_history = store.get(&history.id).unwrap().unwrap();
        assert_eq!(
            linked_history.approved_memory_id.as_deref(),
            Some(approved.id.as_str())
        );
        assert_eq!(linked_history.approved_revision, Some(1));

        let revised = store
            .revise_memory(
                &approved.id,
                NewMemoryRevision {
                    source_text: "Private contract 441".into(),
                    translated_text: "기밀 계약서 441".into(),
                    source_lang: "en".into(),
                    target_lang: "ko".into(),
                    qa_warnings: vec![],
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(revised.revision, 2);
        assert_eq!(revised.revision_kind, "edited");
        let revisions = store.list_memory_revisions(&approved.id).unwrap();
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].translated_text, "기밀 계약서 441");
        assert_eq!(revisions[1].translated_text, "비공개 계약서 441");

        assert!(store.delete(&history.id).unwrap());
        let preserved = store.get_memory(&approved.id).unwrap().unwrap();
        assert!(preserved.history_id.is_none());
        assert_eq!(preserved.revision, 2);
        assert!(store.delete_memory(&approved.id).unwrap());
        assert!(store.get_memory(&approved.id).unwrap().is_none());

        drop(store);
        let bytes = std::fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| std::fs::read(entry.path()).ok())
            .flatten()
            .collect::<Vec<_>>();
        let disk = String::from_utf8_lossy(&bytes);
        assert!(!disk.contains("Private contract"));
        assert!(!disk.contains("기밀 계약서"));
    }
}
