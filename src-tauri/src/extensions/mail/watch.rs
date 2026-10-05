//! Watching a mailbox for new messages (spec 017, US11, FR-056, FR-058): `mail:new-messages
//! {accountId, mailboxName, newCount}` to the frames of the extension that watches.
//!
//! holzi keeps one connection per watch and waits with IMAP IDLE (RFC 2177): the server reports
//! a new message itself. IDLE is renewed every [`IDLE_RENEW`] (servers may end it after 30
//! minutes); a lost connection is opened again after a growing pause. A server without IDLE is
//! asked every `intervalSeconds` instead. Only messages that arrive after the start count. The
//! credentials come with the start call and stay in memory for as long as the watch runs.
//! A watch ends when it is stopped, replaced, with its extension's last frame, when the extension
//! is disabled or removed, and with the process (the vault's end).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::connect::{imap_login, imap_logout, ImapSession};
use super::{check_mailbox, MailError, ServerConfig};
use crate::extensions::bridge::dispatch::Emit;
use crate::extensions::bridge::events::emit_to_frames;
use crate::extensions::host::ExtensionHost;

/// The SDK's event for new messages.
pub const NEW_MESSAGES: &str = "mail:new-messages";

/// How long one IDLE runs before holzi renews it.
pub const IDLE_RENEW: Duration = Duration::from_secs(25 * 60);
/// The bounds of the polling interval of a server without IDLE (the SDK's `intervalSeconds`).
#[cfg(not(test))]
pub const MIN_INTERVAL: Duration = Duration::from_secs(30);
#[cfg(test)]
pub const MIN_INTERVAL: Duration = Duration::from_millis(200);
pub const MAX_INTERVAL: Duration = Duration::from_secs(3600);
/// Watches one extension may run at a time.
pub const MAX_WATCHES: usize = 16;
/// The pauses before opening a lost connection again.
const FIRST_RETRY: Duration = Duration::from_secs(5);
const LAST_RETRY: Duration = Duration::from_secs(300);

type Key = (Uuid, String, String);

/// The running watches of this process.
#[derive(Default)]
pub struct MailWatches {
    running: Mutex<HashMap<Key, CancellationToken>>,
}

impl MailWatches {
    fn lock(&self) -> MutexGuard<'_, HashMap<Key, CancellationToken>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ends the watch of `account`/`mailbox` of `extension_id`; `false` if there was none.
    pub fn stop(&self, extension_id: Uuid, account: &str, mailbox: &str) -> bool {
        let key = (extension_id, account.to_owned(), mailbox.to_owned());
        self.lock().remove(&key).map(|t| t.cancel()).is_some()
    }

    /// Ends every watch of `extension_id`.
    pub fn end_all(&self, extension_id: Uuid) {
        self.lock().retain(|(ext, _, _), token| {
            if *ext == extension_id {
                token.cancel();
                false
            } else {
                true
            }
        });
    }

    pub fn count(&self, extension_id: Uuid) -> usize {
        self.lock()
            .keys()
            .filter(|(ext, _, _)| *ext == extension_id)
            .count()
    }
}

/// What a watch knows about its mailbox: UIDVALIDITY and the highest UID it has reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    validity: u32,
    highest: u32,
}

fn imap_err(error: async_imap::error::Error) -> MailError {
    MailError::Imap(error.to_string())
}

/// Selects `mailbox`; a changed UIDVALIDITY starts counting anew.
async fn select(
    session: &mut ImapSession,
    mailbox: &str,
    seen: &mut Option<Seen>,
) -> Result<(), MailError> {
    let selected = session.select(mailbox).await.map_err(imap_err)?;
    let validity = selected.uid_validity.unwrap_or(0);
    if seen.is_none_or(|s| s.validity != validity) {
        let highest = selected.uid_next.unwrap_or(1).saturating_sub(1);
        *seen = Some(Seen { validity, highest });
    }
    Ok(())
}

/// The number of messages above the highest one reported, which it then moves to.
async fn new_messages(session: &mut ImapSession, seen: &mut Seen) -> Result<u32, MailError> {
    let uids: HashSet<u32> = session
        .uid_search(format!("UID {}:*", seen.highest.saturating_add(1)))
        .await
        .map_err(imap_err)?;
    let newer: Vec<u32> = uids.into_iter().filter(|u| *u > seen.highest).collect();
    if let Some(highest) = newer.iter().max() {
        seen.highest = *highest;
    }
    Ok(newer.len() as u32)
}

/// Where a new message is reported.
struct Report {
    host: Arc<ExtensionHost>,
    emitter: Arc<dyn Emit>,
    extension_id: Uuid,
    account: String,
    mailbox: String,
}

impl Report {
    fn new_messages(&self, count: u32) {
        if count == 0 {
            return;
        }
        emit_to_frames(
            &*self.emitter,
            &self.host,
            self.extension_id,
            NEW_MESSAGES,
            &json!({ "accountId": self.account, "mailboxName": self.mailbox, "newCount": count }),
        );
    }
}

/// One connection's life: log in, select, then IDLE (or poll) until the connection fails or the
/// watch ends. `Ok` only when the watch ended.
async fn connected(
    config: &ServerConfig,
    report: &Report,
    interval: Duration,
    seen: &mut Option<Seen>,
    stop: &CancellationToken,
) -> Result<(), MailError> {
    let mut session = imap_login(config).await?;
    let idle = session
        .capabilities()
        .await
        .map_err(imap_err)?
        .has_str("IDLE");
    select(&mut session, &report.mailbox, seen).await?;
    loop {
        let mut current = seen.unwrap_or(Seen {
            validity: 0,
            highest: 0,
        });
        report.new_messages(new_messages(&mut session, &mut current).await?);
        *seen = Some(current);
        if idle {
            let mut handle = session.idle();
            handle.init().await.map_err(imap_err)?;
            let (waiting, _interrupt) = handle.wait_with_timeout(IDLE_RENEW);
            tokio::select! {
                () = stop.cancelled() => return Ok(()),
                result = waiting => { result.map_err(imap_err)?; }
            }
            session = handle.done().await.map_err(imap_err)?;
        } else {
            tokio::select! {
                () = stop.cancelled() => {
                    imap_logout(session).await;
                    return Ok(());
                }
                () = tokio::time::sleep(interval) => {}
            }
            // NOOP lets the server tell about new messages of the selected mailbox.
            session.noop().await.map_err(imap_err)?;
        }
    }
}

async fn run(config: ServerConfig, report: Report, interval: Duration, stop: CancellationToken) {
    let mut seen = None;
    let mut pause = FIRST_RETRY;
    loop {
        match connected(&config, &report, interval, &mut seen, &stop).await {
            Ok(()) => return,
            Err(error) => log::warn!("mail watch of an extension: {error}; trying again"),
        }
        tokio::select! {
            () = stop.cancelled() => return,
            () = tokio::time::sleep(pause) => {}
        }
        pause = (pause * 2).min(LAST_RETRY);
    }
}

/// Starts (or replaces) the watch of `account`/`mailbox`; it runs on the current runtime.
pub fn start(
    host: &Arc<ExtensionHost>,
    emitter: &Arc<dyn Emit>,
    extension_id: Uuid,
    account: String,
    mailbox: String,
    config: ServerConfig,
    interval: Duration,
) -> Result<(), MailError> {
    check_mailbox(&mailbox)?;
    if account.is_empty() || account.len() > 256 {
        return Err(MailError::Invalid("accountId not allowed".into()));
    }
    super::check_server(&config.host, config.port, config.security)?;
    let watches = &host.mail_watches;
    let key = (extension_id, account.clone(), mailbox.clone());
    let stop = CancellationToken::new();
    {
        let mut running = watches.lock();
        let replacing = running.contains_key(&key);
        if !replacing
            && running
                .keys()
                .filter(|(e, _, _)| *e == extension_id)
                .count()
                >= MAX_WATCHES
        {
            return Err(MailError::TooLarge);
        }
        if let Some(old) = running.insert(key, stop.clone()) {
            old.cancel();
        }
    }
    let report = Report {
        host: Arc::clone(host),
        emitter: Arc::clone(emitter),
        extension_id,
        account,
        mailbox,
    };
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|_| MailError::Invalid("no runtime for the watch".into()))?;
    runtime.spawn(run(
        config,
        report,
        interval.clamp(MIN_INTERVAL, MAX_INTERVAL),
        stop,
    ));
    Ok(())
}
