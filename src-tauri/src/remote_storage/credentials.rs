//! The credentials of a connection in the password manager (spec 038 FR-005, research R2,
//! data-model.md "Zugangsdaten-Eintrag"): an entry owned by `storage`, so rule Z14 of spec 034
//! hides it from every caller but the user and this module. Read and written as
//! `Caller::Internal { feature: "storage" }` with a grant fixed here; new credentials are a new
//! entry (the connection points at it), and an entry no connection uses any more is deleted for
//! good.
//!
//! Whether the entry is on this device ([`state`]): `present`, `missing` when the user deleted it
//! (a delete marker or the trash), otherwise `syncing` (it has not arrived yet).

use haex_crdt::rusqlite::params;
use zeroize::Zeroizing;

use super::{store, Credentials, CredentialsState};
use crate::error::{HolziError, Result};
use crate::passwords::access::{Caller, Grant, GrantAction, Scope};
use crate::passwords::items;
use crate::passwords::model::{ItemInput, KeyValueInput};
use crate::passwords::service::PasswordsService;
use crate::passwords::usage::EntryUsage;
use crate::storage::query::Query;
use crate::vault_gate::VaultDb;

/// The holzi function that owns the entries (`haex_passwords_item_details.owner`).
pub const FEATURE: &str = "storage";

/// The caller this module reads and writes the password manager as.
pub const CALLER: Caller = Caller::Internal { feature: FEATURE };

/// The custom field that holds a session token.
pub const SESSION_TOKEN: &str = "sessionToken";

/// Grants the internal storage caller read and write access for credential operations.
fn grants() -> [Grant; 1] {
    [Grant::new(GrantAction::ReadWrite, Scope::All)]
}

/// Creates the entry for new credentials and returns its id.
pub async fn create(
    passwords: &PasswordsService,
    provider_name: &str,
    endpoint: &str,
    credentials: &Credentials,
) -> Result<String> {
    let input = ItemInput {
        title: Some(format!("S3: {}", provider_name.trim())),
        username: Some(credentials.access_key_id.clone()),
        password: Some(credentials.secret_access_key.to_string()),
        url: (!endpoint.trim().is_empty()).then(|| endpoint.trim().to_owned()),
        key_values: credentials
            .session_token
            .iter()
            .map(|token| KeyValueInput {
                key: SESSION_TOKEN.to_owned(),
                value: Some(token.to_string()),
            })
            .collect(),
        ..ItemInput::default()
    };
    passwords.create_owned_item(&CALLER, input).await
}

/// Reads the credentials of the entry `item_id`. An entry that is not there, or one without a key
/// or a secret, is [`HolziError::StorageCredentialsUnavailable`] with its [`state`].
pub async fn read(
    passwords: &PasswordsService,
    db: &VaultDb,
    item_id: &str,
) -> Result<Credentials> {
    let unavailable = |state| HolziError::StorageCredentialsUnavailable { state };
    let item = match passwords
        .read_secret_item(&CALLER, &grants(), item_id.to_owned())
        .await
    {
        Ok(item) => item,
        Err(HolziError::PasswordsNotFound) => return Err(unavailable(state(db, item_id).await?)),
        Err(error) => return Err(error),
    };
    let session_token = item
        .key_values
        .iter()
        .find(|kv| kv.key.as_deref() == Some(SESSION_TOKEN))
        .and_then(|kv| kv.value.clone())
        .filter(|token| !token.is_empty())
        .map(Zeroizing::new);
    match (item.username, item.password) {
        (Some(access_key_id), Some(secret)) if !access_key_id.is_empty() && !secret.is_empty() => {
            Ok(Credentials {
                access_key_id,
                secret_access_key: Zeroizing::new(secret),
                session_token,
            })
        }
        _ => Err(unavailable(CredentialsState::Missing)),
    }
}

/// Whether the entry `item_id` is here, deleted or not yet arrived (see the module).
pub fn state_in(q: &mut impl Query, item_id: &str) -> Result<CredentialsState> {
    if let Some(stored) = items::item_state(q, item_id)? {
        return Ok(if stored.in_trash {
            CredentialsState::Missing
        } else {
            CredentialsState::Present
        });
    }
    let deleted = q
        .query_row(
            "SELECT COUNT(*) FROM haex_deleted_rows \
             WHERE table_name = 'haex_passwords_item_details' \
               AND json_extract(row_pks, '$.id') = ?1",
            params![item_id],
            |r| r.get::<_, i64>(0),
        )?
        .unwrap_or(0)
        > 0;
    Ok(if deleted {
        CredentialsState::Missing
    } else {
        CredentialsState::Syncing
    })
}

/// [`state_in`] on its own read.
pub async fn state(db: &VaultDb, item_id: &str) -> Result<CredentialsState> {
    let item_id = item_id.to_owned();
    db.read(move |q| state_in(q, &item_id).map_err(Into::into))
        .await
}

/// Deletes the entry for good; one that is already gone is fine.
pub async fn delete(passwords: &PasswordsService, item_id: &str) -> Result<()> {
    match passwords
        .delete_owned_item(&CALLER, item_id.to_owned())
        .await
    {
        Ok(()) | Err(HolziError::PasswordsNotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Tells the password manager that a connection uses an entry (spec 034 FR-034), so deleting
/// "S3: …" there warns first. It reads the vault that is open when it is asked.
pub struct StorageUsage<F> {
    database: F,
}

impl<F> StorageUsage<F>
where
    F: Fn() -> Option<VaultDb> + Send + Sync,
{
    /// Creates a usage check that obtains the active vault from `database` on each request.
    pub fn new(database: F) -> Self {
        Self { database }
    }
}

impl<F> EntryUsage for StorageUsage<F>
where
    F: Fn() -> Option<VaultDb> + Send + Sync,
{
    /// Identifies storage as the feature using the password entry.
    fn feature(&self) -> &'static str {
        FEATURE
    }

    /// Checks whether a connection references the entry; returns false if the vault is unavailable
    /// or the lookup fails.
    fn uses(&self, item_id: &str) -> bool {
        let Some(db) = (self.database)() else {
            return false;
        };
        db.read_blocking(|q| store::credentials_in_use(q, item_id).map_err(Into::into))
            .unwrap_or(false)
    }
}
