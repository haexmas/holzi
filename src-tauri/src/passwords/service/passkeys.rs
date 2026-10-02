//! Passkey methods of the service (spec 034, FR-004): rename and delete, for the user alone
//! (rule Z11). The list of an entry's passkeys is part of the entry detail.

use super::{require_user, PasswordsService};
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::passkeys;

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

    /// Deletes one passkey.
    pub async fn passkey_delete(&self, caller: &Caller, passkey_id: String) -> Result<()> {
        require_user(caller)?;
        self.db()
            .write(move |tx| passkeys::delete(tx, &passkey_id).map_err(Into::into))
            .await
    }
}
