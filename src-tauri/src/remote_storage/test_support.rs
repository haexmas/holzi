//! A [`RemoteStore`] in memory for tests (research R10): objects per bucket, an error to answer
//! per operation, and a record of the calls.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use async_trait::async_trait;
use tokio::time::Instant;
use zeroize::Zeroizing;

use super::{
    Access, Addressing, Credentials, EndpointOrigin, Location, ObjectInfo, ProviderKind,
    RemoteStore, StorageError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    Put,
    Get,
    List,
    Delete,
}

#[derive(Default)]
pub struct FakeStore {
    objects: Mutex<BTreeMap<(String, String), Vec<u8>>>,
    failures: Mutex<HashMap<Op, StorageError>>,
    calls: Mutex<Vec<(Op, String)>>,
}

impl FakeStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every later call of `op` answers `error`.
    pub fn fail(&self, op: Op, error: StorageError) {
        self.failures.lock().expect("lock").insert(op, error);
    }

    /// The keys stored in `bucket`.
    pub fn keys(&self, bucket: &str) -> Vec<String> {
        self.objects
            .lock()
            .expect("lock")
            .keys()
            .filter(|(b, _)| b == bucket)
            .map(|(_, k)| k.clone())
            .collect()
    }

    /// The operations called so far with their key or prefix.
    pub fn calls(&self) -> Vec<(Op, String)> {
        self.calls.lock().expect("lock").clone()
    }

    fn enter(&self, op: Op, key: &str) -> Result<(), StorageError> {
        self.calls.lock().expect("lock").push((op, key.to_owned()));
        match self.failures.lock().expect("lock").get(&op) {
            Some(error) => Err(*error),
            None => Ok(()),
        }
    }
}

fn slot(access: &Access, key: &str) -> (String, String) {
    (access.location.bucket.clone(), key.to_owned())
}

#[async_trait]
impl RemoteStore for FakeStore {
    async fn put(
        &self,
        access: &Access,
        key: &str,
        body: Vec<u8>,
        _deadline: Instant,
    ) -> Result<(), StorageError> {
        self.enter(Op::Put, key)?;
        self.objects
            .lock()
            .expect("lock")
            .insert(slot(access, key), body);
        Ok(())
    }

    async fn get(
        &self,
        access: &Access,
        key: &str,
        max_bytes: usize,
        _deadline: Instant,
    ) -> Result<Vec<u8>, StorageError> {
        self.enter(Op::Get, key)?;
        let body = self
            .objects
            .lock()
            .expect("lock")
            .get(&slot(access, key))
            .cloned()
            .ok_or(StorageError::NotFound)?;
        if body.len() > max_bytes {
            return Err(StorageError::TooLarge);
        }
        Ok(body)
    }

    async fn list(
        &self,
        access: &Access,
        prefix: &str,
        max: usize,
        _deadline: Instant,
    ) -> Result<Vec<ObjectInfo>, StorageError> {
        self.enter(Op::List, prefix)?;
        let objects: Vec<ObjectInfo> = self
            .objects
            .lock()
            .expect("lock")
            .iter()
            .filter(|((b, k), _)| *b == access.location.bucket && k.starts_with(prefix))
            .map(|((_, k), body)| ObjectInfo {
                key: k.clone(),
                size: body.len() as u64,
                last_modified: "2026-10-06T10:00:00.000Z".to_owned(),
            })
            .collect();
        if objects.len() > max {
            return Err(StorageError::TooLarge);
        }
        Ok(objects)
    }

    async fn delete(
        &self,
        access: &Access,
        key: &str,
        _deadline: Instant,
    ) -> Result<(), StorageError> {
        self.enter(Op::Delete, key)?;
        self.objects
            .lock()
            .expect("lock")
            .remove(&slot(access, key));
        Ok(())
    }
}

/// Placeholder credentials; never a real key.
pub fn credentials() -> Credentials {
    Credentials {
        access_key_id: "AKIDEXAMPLE".to_owned(),
        secret_access_key: Zeroizing::new("placeholder-secret".to_owned()),
        session_token: None,
    }
}

/// Access to `bucket` on a local RustFS of the user.
pub fn access(bucket: &str) -> Access {
    Access {
        location: Location {
            provider_kind: ProviderKind::Rustfs,
            endpoint: "http://127.0.0.1:9000".to_owned(),
            endpoint_origin: EndpointOrigin::User,
            region: "us-east-1".to_owned(),
            addressing: Addressing::Path,
            bucket: bucket.to_owned(),
        },
        credentials: credentials(),
    }
}

/// A fresh temporary vault behind a gate; the directory lives as long as the returned guard.
pub fn vault() -> (tempfile::TempDir, crate::vault_gate::VaultDb) {
    let (dir, db) = crate::passwords::test_support::open_test_vault();
    let db = crate::vault_gate::VaultGate::new()
        .vault_db(std::sync::Arc::new(db))
        .expect("open the gate");
    (dir, db)
}

/// Adds an installed extension `name` with a `remoteStorage` permission for `target`.
pub fn grant_storage(db: &crate::vault_gate::VaultDb, name: &str, target: &str, status: &str) {
    let (name, target, status) = (name.to_owned(), target.to_owned(), status.to_owned());
    db.write_blocking(move |tx| {
        let extension_id = format!("ext-{name}");
        tx.execute(
            "INSERT OR IGNORE INTO extensions (id, public_key, name, installed_at, updated_at) \
             VALUES (?1, 'pk', ?2, 0, 0)",
            haex_crdt::rusqlite::params![extension_id, name],
        )?;
        tx.execute(
            "INSERT INTO extension_permissions \
             (id, extension_id, kind, action, target, status, declared, vault_device_uuid, \
              updated_at) VALUES (?1, ?2, 'remoteStorage', 'read', ?3, ?4, 0, ?5, 0)",
            haex_crdt::rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                extension_id,
                target,
                status,
                crate::identity::VAULT_SCOPE_UUID.to_string()
            ],
        )?;
        Ok(())
    })
    .expect("grant a storage permission");
}
