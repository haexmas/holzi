//! Item methods of the service (spec 034, `contracts/access.md` §Dienst): the overview, the
//! entries, the single-entry secret read, reveal, TOTP, copy, create and update. The five item
//! methods are open to every caller that a grant covers (Z3–Z9, Z12, Z13); everything else here is
//! for the user alone (Z11). A caller that is not the user gets only what its grants cover, and
//! never a secret in a list (Z4).

use super::{require_user, PasswordsService};
use crate::error::{HolziError, Result};
use crate::passwords::access::{
    authorize_create, authorize_list, authorize_read, authorize_update, Caller, Grant, ItemState,
    ListView,
};
use crate::passwords::items;
use crate::passwords::model::{
    AgentHeader, CopyField, ItemDetail, ItemHeader, ItemInput, ItemPatch, Overview, RevealedSecret,
    SecretField, SecretItem, TotpCode,
};
use crate::passwords::reveal::{self, Copied};

/// What `list_headers` delivers: the headers of the entries in the scope, or the narrow headers of
/// the built-in agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Headers {
    Items(Vec<ItemHeader>),
    Agent(Vec<AgentHeader>),
}

impl PasswordsService {
    /// The user's overview: every entry (also in the trash), every folder, every tag. Z11.
    pub async fn load_overview(&self, caller: &Caller) -> Result<Overview> {
        require_user(caller)?;
        self.db()
            .read(|q| items::load_overview(q).map_err(Into::into))
            .await
    }

    /// The headers a caller may see (Z2, Z4): the built-in agent gets the narrow headers, everybody
    /// else the entries in its scope, never a secret and never an entry in the trash (the window
    /// reads the trash through [`Self::load_overview`]).
    pub async fn list_headers(&self, caller: &Caller, grants: &[Grant]) -> Result<Headers> {
        let view = authorize_list(caller, grants)?;
        self.db()
            .read(move |q| {
                Ok(match view {
                    ListView::Agent => Headers::Agent(items::agent_headers(q)?),
                    ListView::Items(scope) => Headers::Items(items::headers_in_scope(q, &scope)?),
                })
            })
            .await
    }

    /// The entry without secrets, for the window. Z11 (the other callers read through
    /// [`Self::read_secret_item`]).
    pub async fn get_item(&self, caller: &Caller, item_id: String) -> Result<ItemDetail> {
        require_user(caller)?;
        self.db()
            .read(move |q| {
                items::get_item(q, &item_id)?
                    .ok_or(HolziError::PasswordsNotFound)
                    .map_err(Into::into)
            })
            .await
    }

    /// The entry with its secrets on a single request (FR-026). An entry outside the scope or in
    /// the trash is `PasswordsNotFound`, the same as a missing one (Z5, Z13).
    pub async fn read_secret_item(
        &self,
        caller: &Caller,
        grants: &[Grant],
        item_id: String,
    ) -> Result<SecretItem> {
        let (caller, grants) = (caller.clone(), grants.to_vec());
        self.db()
            .read(move |q| {
                let Some((tags, in_trash)) = items::item_state(q, &item_id)? else {
                    // Forbidden without a read grant, otherwise indistinguishable from outside the
                    // scope (Z3, Z5).
                    authorize_read(
                        &caller,
                        &grants,
                        &ItemState {
                            tags: &[],
                            in_trash: false,
                        },
                    )
                    .map_err(HolziError::from)?;
                    return Err(HolziError::PasswordsNotFound.into());
                };
                authorize_read(
                    &caller,
                    &grants,
                    &ItemState {
                        tags: &tags,
                        in_trash,
                    },
                )
                .map_err(HolziError::from)?;
                reveal::secret_item(q, &item_id)?
                    .ok_or(HolziError::PasswordsNotFound)
                    .map_err(Into::into)
            })
            .await
    }

    /// The value of a secret for the moment the user asks to see it. Z11.
    pub async fn reveal(
        &self,
        caller: &Caller,
        item_id: String,
        field: SecretField,
    ) -> Result<RevealedSecret> {
        require_user(caller)?;
        self.db()
            .read(move |q| reveal::reveal(q, &item_id, &field).map_err(Into::into))
            .await
    }

    /// The current TOTP code of an entry and the seconds until it changes. Z11.
    pub async fn totp_code(&self, caller: &Caller, item_id: String) -> Result<TotpCode> {
        require_user(caller)?;
        self.db()
            .read(move |q| reveal::totp_code(q, &item_id, reveal::unix_now()).map_err(Into::into))
            .await
    }

    /// The value for the clipboard; the command that called this writes it there. Z11.
    pub async fn copy_value(
        &self,
        caller: &Caller,
        item_id: String,
        field: CopyField,
    ) -> Result<Copied> {
        require_user(caller)?;
        self.db()
            .read(move |q| reveal::copy_value(q, &item_id, &field).map_err(Into::into))
            .await
    }

    /// How long a copy stays on the clipboard (the vault setting), `None` when it is off. Z11.
    pub async fn clipboard_delay(&self, caller: &Caller) -> Result<Option<std::time::Duration>> {
        require_user(caller)?;
        self.db()
            .read(|q| crate::passwords::settings::clear_after(q).map_err(Into::into))
            .await
    }

    /// Creates an entry. A caller with a tag scope must send only tags of its scope and at least
    /// one (Z6).
    pub async fn create_item(
        &self,
        caller: &Caller,
        grants: &[Grant],
        input: ItemInput,
        group_id: Option<String>,
    ) -> Result<String> {
        authorize_create(caller, grants, &input.tags)?;
        self.db()
            .write(move |tx| {
                items::create_item(tx, &input, group_id.as_deref()).map_err(Into::into)
            })
            .await
    }

    /// Applies a partial update if the token `expected_updated_at` is still current. For a caller
    /// with a tag scope the entry must lie in the scope before and after, tags outside the scope
    /// stay as they are and none can be added (Z7, Z12).
    pub async fn update_item(
        &self,
        caller: &Caller,
        grants: &[Grant],
        item_id: String,
        expected_updated_at: String,
        mut patch: ItemPatch,
    ) -> Result<String> {
        let (caller, grants) = (caller.clone(), grants.to_vec());
        self.db()
            .write(move |tx| {
                let state = items::item_state(tx, &item_id)?;
                match (&state, &caller) {
                    (Some((tags, in_trash)), _) => {
                        let item = ItemState {
                            tags,
                            in_trash: *in_trash,
                        };
                        let result =
                            authorize_update(&caller, &grants, &item, patch.tags.as_deref())
                                .map_err(HolziError::from)?;
                        if patch.tags.is_some() {
                            patch.tags = Some(result);
                        }
                    }
                    // A missing entry is a conflict for the user and unknown to everybody else.
                    (None, Caller::User) => {}
                    (None, _) => {
                        // Forbidden without a write grant, otherwise not found (Z3, Z5).
                        authorize_update(
                            &caller,
                            &grants,
                            &ItemState {
                                tags: &[],
                                in_trash: false,
                            },
                            None,
                        )
                        .map_err(HolziError::from)?;
                        return Err(HolziError::PasswordsNotFound.into());
                    }
                }
                items::update_item(tx, &item_id, &expected_updated_at, &patch).map_err(Into::into)
            })
            .await
    }
}
