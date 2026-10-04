//! Import methods of the service (spec 034, US7, FR-023): preview, run and the picture of an
//! imported icon. For the user alone (rule Z11). The file is read and parsed on blocking threads
//! (a KeePass key derivation can take seconds); the parse is a pure function of the bytes.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use haex_crdt::rusqlite::params;
use zeroize::Zeroizing;

use super::{require_user, PasswordsService};
use crate::error::{HolziError, Result};
use crate::passwords::access::Caller;
use crate::passwords::import::apply::{self, Control, OnDuplicate, Progress};
use crate::passwords::import::haex_vault::{self, WARNING_CLOSE_FIRST, WARNING_HAEX_PASS_TABLES};
use crate::passwords::import::{self, Credentials, ExistingKeys, ImportModel, ImportSource};
use crate::passwords::model::{ImportPreview, ImportReport};
use crate::passwords::TRASH_GROUP_ID;
use crate::storage::query::Query;

fn failed(reason: &str) -> HolziError {
    HolziError::PasswordsImportFailed {
        reason: reason.to_string(),
    }
}

/// What the window hands over for a run: the format, the file, and a KeePass password and key file
/// path. The secrets are zeroed when dropped.
pub struct ImportRequest {
    pub source: ImportSource,
    pub path: String,
    pub password: Option<Zeroizing<String>>,
    pub key_file_path: Option<String>,
}

/// A file read: its model and what the preview says about the file besides the counts.
struct ReadFile {
    model: ImportModel,
    warnings: Vec<String>,
}

/// Reads the file (and key file) and parses it, off the async threads. A haex-vault file is a
/// database and is read from its path (spec 037).
async fn read_model(request: ImportRequest) -> Result<ReadFile> {
    tauri::async_runtime::spawn_blocking(move || {
        if request.source == ImportSource::HaexVault {
            let credentials = Credentials {
                password: request.password,
                key_file: None,
            };
            let read = haex_vault::read(Path::new(&request.path), &credentials)?;
            let mut warnings = vec![WARNING_CLOSE_FIRST.to_string()];
            if read.has_haex_pass_tables {
                warnings.push(WARNING_HAEX_PASS_TABLES.to_string());
            }
            return Ok(ReadFile {
                model: read.model,
                warnings,
            });
        }
        let bytes = Zeroizing::new(
            std::fs::read(PathBuf::from(&request.path)).map_err(|_| failed("unreadable"))?,
        );
        let key_file = match &request.key_file_path {
            Some(path) => Some(Zeroizing::new(
                std::fs::read(PathBuf::from(path)).map_err(|_| failed("unreadable"))?,
            )),
            None => None,
        };
        let credentials = Credentials {
            password: request.password,
            key_file,
        };
        Ok(ReadFile {
            model: import::parse(request.source, &bytes, &credentials)?,
            warnings: Vec::new(),
        })
    })
    .await
    .map_err(|_| failed("unreadable"))?
}

impl PasswordsService {
    /// The (title, user name, address) of every entry outside the trash, for duplicate detection.
    async fn existing_keys(&self) -> Result<ExistingKeys> {
        self.db()
            .read(|q| {
                let rows: Vec<(Option<String>, Option<String>, Option<String>)> = q.query_map(
                    "WITH RECURSIVE trashed(id) AS ( \
                         SELECT ?1 \
                         UNION ALL \
                         SELECT g.id FROM haex_passwords_groups g \
                         JOIN trashed t ON g.parent_id = t.id) \
                     SELECT d.title, d.username, d.url FROM haex_passwords_item_details d \
                     WHERE d.id NOT IN (SELECT item_id FROM haex_passwords_group_items \
                                        WHERE group_id IN (SELECT id FROM trashed))",
                    params![TRASH_GROUP_ID],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?;
                Ok(rows
                    .into_iter()
                    .map(|(t, u, l)| {
                        import::duplicate_key(t.as_deref(), u.as_deref(), l.as_deref())
                    })
                    .collect())
            })
            .await
    }

    /// Counts what the file would bring over; writes nothing.
    pub async fn import_preview(
        &self,
        caller: &Caller,
        request: ImportRequest,
    ) -> Result<ImportPreview> {
        require_user(caller)?;
        let read = read_model(request).await?;
        let existing = self.existing_keys().await?;
        let mut preview = import::preview(&read.model, &existing);
        preview.warnings.splice(0..0, read.warnings);
        Ok(preview)
    }

    /// Writes the file in steps; see [`apply::run`]. `cancel` is the flag of the run's slot.
    pub async fn import_run(
        &self,
        caller: &Caller,
        request: ImportRequest,
        on_duplicate: OnDuplicate,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Send + Sync),
    ) -> Result<ImportReport> {
        require_user(caller)?;
        let model = read_model(request).await?.model;
        let existing = self.existing_keys().await?;
        let control = Control {
            cancel,
            progress,
            inject: None,
        };
        apply::run(self.db(), model, &existing, on_duplicate, &control).await
    }

    /// The bytes of an imported picture (`icon = 'binary:<hash>'`).
    pub async fn icon_preview(&self, caller: &Caller, hash: String) -> Result<Vec<u8>> {
        require_user(caller)?;
        self.db()
            .read(move |q| {
                q.query_row(
                    "SELECT data FROM haex_passwords_binaries WHERE hash = ?1 AND type = 'icon'",
                    params![hash],
                    |r| r.get::<_, Vec<u8>>(0),
                )?
                .ok_or(HolziError::PasswordsNotFound)
                .map_err(Into::into)
            })
            .await
    }
}
