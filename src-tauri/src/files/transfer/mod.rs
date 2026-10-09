//! Transfers of the file browser (spec 044 FR-018 to FR-026, data-model.md `Transfer`): copy, move
//! and delete run on the blocking pool as tracked work of the vault session, each with its own
//! token below the gate's, so closing the vault cancels them (FR-026, ADR 0003). The window follows
//! a transfer through a [`Channel`] and answers a name conflict with [`TransferManager::answer`].

pub mod local;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::files::{FilesError, FilesErrorCode};
use crate::vault_gate::VaultGate;
use local::{Hooks, Job, Outcome, Removal, Totals};

/// What a transfer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub enum TransferOp {
    Copy,
    Move,
    Delete,
}

/// The answer to a name that is taken at the target (FR-021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub enum ConflictChoice {
    Replace,
    KeepBoth,
    Skip,
}

/// How far a transfer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct TransferProgress {
    #[ts(type = "number")]
    pub items_done: u64,
    #[ts(type = "number")]
    pub items_total: u64,
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
}

/// What the window hears of a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub enum TransferEvent {
    Progress {
        progress: TransferProgress,
    },
    /// A name is taken; the transfer waits for [`TransferManager::answer`].
    Conflict {
        name: String,
    },
    Done,
    Cancelled,
    /// May start again with [`TransferManager::retry`].
    Failed {
        error: FilesError,
    },
}

/// How often progress goes to the window at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// How often a transfer waiting for an answer looks whether it was cancelled.
const ANSWER_POLL: Duration = Duration::from_millis(100);

/// A transfer the manager knows: running, or failed and able to start again.
struct Known {
    job: Job,
    totals: Totals,
    removal: Removal,
    channel: Channel<TransferEvent>,
    cancel: CancellationToken,
    answers: Sender<(ConflictChoice, bool)>,
}

/// The transfers of this session; a Tauri managed state.
#[derive(Clone, Default)]
pub struct TransferManager {
    known: Arc<Mutex<HashMap<String, Known>>>,
}

impl TransferManager {
    fn known(&self) -> MutexGuard<'_, HashMap<String, Known>> {
        self.known.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts a prepared job; returns its id.
    pub fn start(
        &self,
        gate: &VaultGate,
        job: Job,
        totals: Totals,
        removal: Removal,
        channel: Channel<TransferEvent>,
    ) -> Result<String, FilesError> {
        let id = uuid::Uuid::new_v4().to_string();
        self.launch(gate, &id, job, totals, removal, channel)?;
        Ok(id)
    }

    fn launch(
        &self,
        gate: &VaultGate,
        id: &str,
        job: Job,
        totals: Totals,
        removal: Removal,
        channel: Channel<TransferEvent>,
    ) -> Result<(), FilesError> {
        let cancel = gate.token().child_token();
        let (answers, answered) = mpsc::channel();
        self.known().insert(
            id.to_owned(),
            Known {
                job: job.clone(),
                totals,
                removal,
                channel: channel.clone(),
                cancel: cancel.clone(),
                answers,
            },
        );
        let manager = self.clone();
        let owned_id = id.to_owned();
        let spawned = gate.spawn_blocking(move || {
            let outcome = work(&job, &totals, removal, &cancel, &channel, &answered);
            manager.finish(&owned_id, outcome, &channel);
        });
        if let Err(error) = spawned {
            self.known().remove(id);
            log::warn!("files: a transfer could not start: {error}");
            return Err(FilesError::new(
                FilesErrorCode::Unsupported,
                "the vault is closing",
            ));
        }
        Ok(())
    }

    /// Reports the end; a failed transfer stays known for [`Self::retry`].
    fn finish(&self, id: &str, outcome: Outcome, channel: &Channel<TransferEvent>) {
        let event = match outcome {
            Outcome::Done => TransferEvent::Done,
            Outcome::Cancelled => TransferEvent::Cancelled,
            Outcome::Failed(error) => TransferEvent::Failed { error },
        };
        if !matches!(event, TransferEvent::Failed { .. }) {
            self.known().remove(id);
        }
        let _ = channel.send(event);
    }

    /// Answers the conflict a transfer waits on.
    pub fn answer(&self, id: &str, choice: ConflictChoice, for_all: bool) {
        if let Some(known) = self.known().get(id) {
            let _ = known.answers.send((choice, for_all));
        }
    }

    /// Cancels a running transfer, or forgets a failed one.
    pub fn cancel(&self, id: &str) {
        if let Some(known) = self.known().remove(id) {
            known.cancel.cancel();
        }
    }

    /// Starts a failed transfer again with the same job and channel.
    pub fn retry(&self, gate: &VaultGate, id: &str) -> Result<(), FilesError> {
        let Some(known) = self.known().remove(id) else {
            return Err(FilesError::new(
                FilesErrorCode::NotFound,
                "no such transfer",
            ));
        };
        self.launch(
            gate,
            id,
            known.job,
            known.totals,
            known.removal,
            known.channel,
        )
    }
}

/// The blocking part: runs the job, sends progress at most every [`PROGRESS_EVERY`] and asks the
/// window about conflicts.
fn work(
    job: &Job,
    totals: &Totals,
    removal: Removal,
    cancel: &CancellationToken,
    channel: &Channel<TransferEvent>,
    answered: &Receiver<(ConflictChoice, bool)>,
) -> Outcome {
    let mut last = Instant::now() - PROGRESS_EVERY;
    let mut progress = |progress: TransferProgress| {
        let finished = progress.items_done == progress.items_total;
        if finished || last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            let _ = channel.send(TransferEvent::Progress { progress });
        }
    };
    let mut ask = |name: &str| {
        let _ = channel.send(TransferEvent::Conflict {
            name: name.to_owned(),
        });
        loop {
            match answered.recv_timeout(ANSWER_POLL) {
                Ok(answer) => return Some(answer),
                Err(RecvTimeoutError::Timeout) if !cancel.is_cancelled() => {}
                Err(_) => return None,
            }
        }
    };
    local::run(
        job,
        totals,
        Hooks {
            cancel,
            ask: &mut ask,
            progress: &mut progress,
            removal,
            rename: &|from, to| std::fs::rename(from, to),
        },
    )
}

/// How a delete removes on this platform: the trash on desktops, for good on mobiles (FR-023).
pub fn platform_removal() -> Removal {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        Removal::Permanent
    } else {
        Removal::Trash
    }
}

/// Whether two paths are on one file system (a move between them is a rename).
pub fn same_device(a: &std::path::Path, b: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::symlink_metadata(a), std::fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev(),
            _ => true,
        }
    }
    #[cfg(not(unix))]
    {
        a.components().next() == b.components().next()
    }
}
