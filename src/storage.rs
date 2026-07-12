use std::{
    path::Path,
    sync::mpsc::{self, Sender},
    thread,
};

use anyhow::{Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    crypto::VaultCrypto,
    history::{HistoryFilter, HistoryRecord, HistoryStore, NewHistoryRecord},
    translation_memory::{NewMemoryRevision, TranslationMemoryRecord},
};

#[derive(Clone)]
pub struct StorageWorker {
    sender: Sender<StorageCommand>,
}

enum StorageCommand {
    Count(oneshot::Sender<Result<u64>>),
    Insert(NewHistoryRecord, oneshot::Sender<Result<HistoryRecord>>),
    List(HistoryFilter, oneshot::Sender<Result<Vec<HistoryRecord>>>),
    Get(String, oneshot::Sender<Result<Option<HistoryRecord>>>),
    SetFavorite(String, bool, oneshot::Sender<Result<bool>>),
    Delete(String, oneshot::Sender<Result<bool>>),
    ApproveHistory(
        String,
        oneshot::Sender<Result<Option<TranslationMemoryRecord>>>,
    ),
    GetMemory(
        String,
        oneshot::Sender<Result<Option<TranslationMemoryRecord>>>,
    ),
    ListMemory(usize, oneshot::Sender<Result<Vec<TranslationMemoryRecord>>>),
    ReviseMemory(
        String,
        NewMemoryRevision,
        oneshot::Sender<Result<Option<TranslationMemoryRecord>>>,
    ),
    ListMemoryRevisions(
        String,
        oneshot::Sender<Result<Vec<TranslationMemoryRecord>>>,
    ),
    DeleteMemory(String, oneshot::Sender<Result<bool>>),
}

impl StorageWorker {
    pub fn start(path: &Path, crypto: VaultCrypto) -> Result<Self> {
        let store = HistoryStore::open(path, crypto)?;
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("private-translator-storage".into())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    match command {
                        StorageCommand::Count(reply) => {
                            let _ = reply.send(store.count());
                        }
                        StorageCommand::Insert(record, reply) => {
                            let _ = reply.send(store.insert(record));
                        }
                        StorageCommand::List(filter, reply) => {
                            let _ = reply.send(store.list(filter));
                        }
                        StorageCommand::Get(id, reply) => {
                            let _ = reply.send(store.get(&id));
                        }
                        StorageCommand::SetFavorite(id, favorite, reply) => {
                            let _ = reply.send(store.set_favorite(&id, favorite));
                        }
                        StorageCommand::Delete(id, reply) => {
                            let _ = reply.send(store.delete(&id));
                        }
                        StorageCommand::ApproveHistory(id, reply) => {
                            let _ = reply.send(store.approve_history(&id));
                        }
                        StorageCommand::GetMemory(id, reply) => {
                            let _ = reply.send(store.get_memory(&id));
                        }
                        StorageCommand::ListMemory(limit, reply) => {
                            let _ = reply.send(store.list_memory(limit));
                        }
                        StorageCommand::ReviseMemory(id, revision, reply) => {
                            let _ = reply.send(store.revise_memory(&id, revision));
                        }
                        StorageCommand::ListMemoryRevisions(id, reply) => {
                            let _ = reply.send(store.list_memory_revisions(&id));
                        }
                        StorageCommand::DeleteMemory(id, reply) => {
                            let _ = reply.send(store.delete_memory(&id));
                        }
                    }
                }
            })
            .map_err(|error| anyhow!("Could not start the local storage worker: {error}"))?;
        Ok(Self { sender })
    }

    pub async fn count(&self) -> Result<u64> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::Count(reply))?;
        receive(response).await
    }

    pub async fn insert(&self, record: NewHistoryRecord) -> Result<HistoryRecord> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::Insert(record, reply))?;
        receive(response).await
    }

    pub async fn list(&self, filter: HistoryFilter) -> Result<Vec<HistoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::List(filter, reply))?;
        receive(response).await
    }

    pub async fn get(&self, id: &str) -> Result<Option<HistoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::Get(id.to_owned(), reply))?;
        receive(response).await
    }

    pub async fn set_favorite(&self, id: &str, favorite: bool) -> Result<bool> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::SetFavorite(id.to_owned(), favorite, reply))?;
        receive(response).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::Delete(id.to_owned(), reply))?;
        receive(response).await
    }

    pub async fn approve_history(&self, id: &str) -> Result<Option<TranslationMemoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::ApproveHistory(id.to_owned(), reply))?;
        receive(response).await
    }

    pub async fn get_memory(&self, id: &str) -> Result<Option<TranslationMemoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::GetMemory(id.to_owned(), reply))?;
        receive(response).await
    }

    pub async fn list_memory(&self, limit: usize) -> Result<Vec<TranslationMemoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::ListMemory(limit, reply))?;
        receive(response).await
    }

    pub async fn revise_memory(
        &self,
        id: &str,
        revision: NewMemoryRevision,
    ) -> Result<Option<TranslationMemoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::ReviseMemory(id.to_owned(), revision, reply))?;
        receive(response).await
    }

    pub async fn list_memory_revisions(&self, id: &str) -> Result<Vec<TranslationMemoryRecord>> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::ListMemoryRevisions(id.to_owned(), reply))?;
        receive(response).await
    }

    pub async fn delete_memory(&self, id: &str) -> Result<bool> {
        let (reply, response) = oneshot::channel();
        self.send(StorageCommand::DeleteMemory(id.to_owned(), reply))?;
        receive(response).await
    }

    fn send(&self, command: StorageCommand) -> Result<()> {
        self.sender
            .send(command)
            .map_err(|_| anyhow!("The local storage worker stopped"))
    }
}

async fn receive<T>(response: oneshot::Receiver<Result<T>>) -> Result<T> {
    response
        .await
        .map_err(|_| anyhow!("The local storage worker response was interrupted"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NewHistoryRecord {
        NewHistoryRecord {
            source_text: "Private draft 808".into(),
            translated_text: "비공개 초안 808".into(),
            source_lang: "en".into(),
            target_lang: "ko".into(),
            model_id: "test".into(),
            model_label: "Test".into(),
            mode: "test".into(),
            privacy: "device".into(),
            latency_ms: 12,
            qa_warnings: vec![],
        }
    }

    #[tokio::test]
    async fn worker_runs_database_operations_off_the_async_executor() {
        let temporary = tempfile::tempdir().unwrap();
        let crypto = VaultCrypto::from_key(&[7_u8; 32]).unwrap();
        let worker = StorageWorker::start(&temporary.path().join("history.db"), crypto).unwrap();

        let inserted = worker.insert(sample()).await.unwrap();
        assert_eq!(worker.count().await.unwrap(), 1);
        assert_eq!(
            worker.get(&inserted.id).await.unwrap().unwrap().source_text,
            "Private draft 808"
        );
        assert!(worker.set_favorite(&inserted.id, true).await.unwrap());
        let approved = worker.approve_history(&inserted.id).await.unwrap().unwrap();
        assert_eq!(worker.list_memory(20).await.unwrap().len(), 1);
        let revised = worker
            .revise_memory(
                &approved.id,
                NewMemoryRevision {
                    source_text: "Private draft 808".into(),
                    translated_text: "기밀 초안 808".into(),
                    source_lang: "en".into(),
                    target_lang: "ko".into(),
                    qa_warnings: vec![],
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(revised.revision, 2);
        assert_eq!(
            worker
                .get_memory(&approved.id)
                .await
                .unwrap()
                .unwrap()
                .translated_text,
            "기밀 초안 808"
        );
        assert_eq!(
            worker
                .list_memory_revisions(&approved.id)
                .await
                .unwrap()
                .len(),
            2
        );
        assert!(worker.delete(&inserted.id).await.unwrap());
        assert_eq!(worker.count().await.unwrap(), 0);
    }
}
