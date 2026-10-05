//! Passkey methods of the service. For the user alone (rule Z11): rename, delete (spec 034,
//! FR-004) and unlink (spec 036, research R6). For any caller with its grants: create, confirm and
//! list (spec 036, US5, `contracts/passkey-service.md`); those three have no Tauri command, the
//! window neither creates nor uses a passkey (FR-023). The list of an entry's passkeys is part of
//! the entry detail.

use super::{require_user, PasswordsService};
use crate::error::Result;
use crate::passwords::access::{Caller, Grant};
use crate::passwords::model_passkeys::{
    PasskeyAssertion, PasskeyConfirmRequest, PasskeyCreateRequest, PasskeyCreated, PasskeyHeader,
    PasskeyListRequest,
};
use crate::passwords::references_db::Reader;
use crate::passwords::{passkey_links, passkeys, passkeys_ops};

impl PasswordsService {
    /// Sets the nickname of a passkey.
    pub async fn passkey_rename(
        &self,
        caller: &Caller,
        passkey_id: String,
        nickname: Option<String>,
    ) -> Result<()> {
        require_user(caller)?;
        self.db()
            .write(move |tx| {
                passkeys::rename(tx, &passkey_id, nickname.as_deref()).map_err(Into::into)
            })
            .await
    }

    /// Deletes one passkey and its links to other entries.
    pub async fn passkey_delete(&self, caller: &Caller, passkey_id: String) -> Result<()> {
        require_user(caller)?;
        self.db()
            .write(move |tx| passkeys::delete(tx, &passkey_id).map_err(Into::into))
            .await
    }

    /// Drops the link that shows `passkey_id` at `item_id` ("Verweis lösen"); the passkey stays.
    pub async fn passkey_unlink(
        &self,
        caller: &Caller,
        item_id: String,
        passkey_id: String,
    ) -> Result<()> {
        require_user(caller)?;
        self.db()
            .write(move |tx| passkey_links::unlink(tx, &item_id, &passkey_id).map_err(Into::into))
            .await
    }

    /// A new passkey at an entry the caller may write (`contracts/passkey-service.md`).
    pub async fn passkey_create(
        &self,
        caller: &Caller,
        grants: &[Grant],
        request: PasskeyCreateRequest,
    ) -> Result<PasskeyCreated> {
        let (caller, grants) = (caller.clone(), grants.to_vec());
        self.db()
            .write(move |tx| {
                let reader = Reader {
                    caller: &caller,
                    grants: &grants,
                };
                passkeys_ops::create(tx, reader, &request).map_err(Into::into)
            })
            .await
    }

    /// Signs a challenge with the one passkey that fits (`contracts/passkey-service.md`).
    pub async fn passkey_confirm(
        &self,
        caller: &Caller,
        grants: &[Grant],
        request: PasskeyConfirmRequest,
    ) -> Result<PasskeyAssertion> {
        let (caller, grants) = (caller.clone(), grants.to_vec());
        self.db()
            .write(move |tx| {
                let reader = Reader {
                    caller: &caller,
                    grants: &grants,
                };
                passkeys_ops::confirm(tx, reader, &request).map_err(Into::into)
            })
            .await
    }

    /// The headers of the passkeys the caller reaches (`contracts/passkey-service.md`).
    pub async fn passkey_list(
        &self,
        caller: &Caller,
        grants: &[Grant],
        request: PasskeyListRequest,
    ) -> Result<Vec<PasskeyHeader>> {
        let (caller, grants) = (caller.clone(), grants.to_vec());
        self.db()
            .read(move |q| {
                let reader = Reader {
                    caller: &caller,
                    grants: &grants,
                };
                passkeys_ops::list(q, reader, &request).map_err(Into::into)
            })
            .await
    }
}
