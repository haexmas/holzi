//! The exchange with one connected device (contracts/sync-protocol.md §2,
//! FR-012, FR-019, research R4).
//!
//! After the handshake its stream stays open as the control stream: each
//! side sends `Progress` on it, at the start and whenever its own progress
//! moved (a local commit or an applied pull). Whoever sees the other side
//! further for any origin opens a new stream and pulls; there is at most
//! one pull per connection and direction at a time. Data never flows
//! without a `Pull`.

use std::collections::BTreeSet;
use std::sync::Arc;

use iroh::endpoint::{Connection, RecvStream, SendStream};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::sync::content_keys;
use crate::sync::device_list;
use crate::sync::handshake::Peer;
use crate::sync::inbound::park::refetch::Floor;
use crate::sync::inbound::Inbox;
use crate::sync::keys::DeviceKeys;
use crate::sync::outbound::{serve, Served};
use crate::sync::progress::{self, Vector};
use crate::sync::replica::Replica;
use crate::sync::resync;
use crate::sync::seen;
use crate::sync::wire::{
    expect_frame, read_frame, write_frame, ErrorCode, Message, ResyncReason, FRAME_LIMIT,
};

/// What a session needs from the node.
pub struct SessionContext {
    pub replica: Arc<Replica>,
    pub keys: Arc<DeviceKeys>,
    pub vault: [u8; 32],
    /// Moves whenever this device's progress may have moved.
    pub changed: watch::Receiver<u64>,
    /// Signals that this device's progress moved, to every session.
    pub bump: Arc<watch::Sender<u64>>,
    /// Receives the tables a pull changed.
    pub on_applied: Arc<dyn Fn(BTreeSet<String>) + Send + Sync>,
    /// Notifies the device view when a reported last-seen value advances.
    pub on_devices_changed: Arc<dyn Fn() + Send + Sync>,
    pub cancel: CancellationToken,
}

/// Why a session ended.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Wire(#[from] crate::sync::wire::WireError),
    #[error(transparent)]
    Inbound(#[from] crate::sync::inbound::InboundError),
    #[error(transparent)]
    Crdt(#[from] haex_crdt::Error),
    #[error("the connection failed: {0}")]
    Connection(#[from] iroh::endpoint::ConnectionError),
    #[error("a stream failed: {0}")]
    Stream(String),
    #[error("a database task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("session protocol violation: {0}")]
    Protocol(&'static str),
    /// The device list this device holds no longer names the peer, or
    /// removes it (FR-027).
    #[error("the peer is no longer a device of the vault")]
    NoLongerListed,
}

impl SessionError {
    /// Whether the vault's close ended the session, which is no fault of it.
    fn is_closing(&self) -> bool {
        matches!(
            self,
            SessionError::Inbound(crate::sync::inbound::InboundError::Closing(_))
        )
    }

    fn code(&self) -> ErrorCode {
        match self {
            SessionError::Wire(e) => e.code(),
            _ if self.is_closing() => ErrorCode::Closed,
            SessionError::Inbound(_) => ErrorCode::PullFailed,
            SessionError::Protocol(_) => ErrorCode::Protocol,
            SessionError::NoLongerListed => ErrorCode::Rejected,
            _ => ErrorCode::Closed,
        }
    }
}

/// Runs the exchange until the connection or the session ends.
pub async fn run(
    ctx: SessionContext,
    connection: Connection,
    control_send: SendStream,
    control_recv: RecvStream,
    peer: Peer,
) {
    let ctx = Arc::new(ctx);
    // Best-effort catch-up: a prior session's pull() can be dropped by this
    // same select! after a page with envelope/device-list rows already
    // committed but before they were unwrapped (the pull future simply loses
    // the race and never reaches the end of its loop). unwrap_envelopes is
    // idempotent, so retrying it here recovers that state on the next
    // session with any peer instead of leaving it stuck forever.
    if let Err(error) = unwrap_envelopes(&ctx).await {
        log::warn!("sync: unwrapping pending envelopes at session start failed: {error}");
    }
    let (theirs_tx, theirs_rx) = watch::channel(Vector::new());
    let result = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => Ok(()),
        error = connection.closed() => {
            log::debug!("sync: the connection ended: {error}");
            Ok(())
        }
        result = read_progress(&ctx, control_recv, theirs_tx, &peer) => result,
        result = send_progress(&ctx, control_send, &peer) => result,
        result = pull_when_behind(&ctx, &connection, theirs_rx, &peer) => result,
        result = serve_pulls(&ctx, &connection, &peer) => result,
    };
    match result {
        Ok(()) => connection.close(ErrorCode::Closed.as_u32().into(), b"closed"),
        Err(error) => {
            let level = if error.is_closing() {
                log::Level::Debug
            } else {
                log::Level::Warn
            };
            log::log!(
                level,
                "sync: the session with {} ended: {error}",
                crate::sync::keys::hex(&peer.device_pubkey[..4])
            );
            connection.close(error.code().as_u32().into(), b"error");
        }
    }
}

/// Keeps the other side's latest progress, and what it knows of when
/// devices were last online (FR-033).
async fn read_progress(
    ctx: &SessionContext,
    mut recv: RecvStream,
    theirs: watch::Sender<Vector>,
    peer: &Peer,
) -> Result<(), SessionError> {
    while let Some(message) = read_frame(&mut recv, FRAME_LIMIT).await? {
        match message {
            Message::Progress { vector, last_seen } => {
                // A removed device's reports are not taken in.
                ensure_listed(ctx, peer).await?;
                theirs.send_replace(vector);
                merge_last_seen(ctx, last_seen).await;
            }
            // Pushes after the handshake are for user story 2.
            Message::DeviceListPush { .. } => {}
            _ => return Err(SessionError::Protocol("an unexpected control message")),
        }
    }
    Ok(())
}

/// Takes over the times a peer reports. Best-effort: a failure only leaves
/// "last online" a little older.
async fn merge_last_seen(ctx: &SessionContext, reports: Vec<([u8; 32], u64)>) {
    if reports.is_empty() {
        return;
    }
    let replica = Arc::clone(&ctx.replica);
    let own = ctx.keys.device_pubkey;
    match tokio::task::spawn_blocking(move || seen::merge(&replica, &reports, &own, now_ms())).await
    {
        Ok(Ok(true)) => (ctx.on_devices_changed)(),
        Ok(Ok(false)) => {}
        Ok(Err(error)) => log::debug!("sync: merging last-seen times failed: {error}"),
        Err(error) => log::debug!("sync: merging last-seen times did not run: {error}"),
    }
}

/// Sends this device's progress at the start and whenever it moved.
///
/// Every change can be a device list that removes the peer, learned from
/// another device. A removed peer that never pulls would otherwise keep the
/// session and get the version vector and last-seen times of every device.
async fn send_progress(
    ctx: &SessionContext,
    mut send: SendStream,
    peer: &Peer,
) -> Result<(), SessionError> {
    let mut changed = ctx.changed.clone();
    let mut last_sent: Option<Vector> = None;
    loop {
        changed.mark_unchanged();
        let own = own_progress(&ctx.replica).await?;
        let message = if last_sent.as_ref() != Some(&own) {
            Some(Message::Progress {
                vector: own.clone(),
                last_seen: last_seen_report(ctx).await,
            })
        } else {
            None
        };
        // Checked after the data is read, as in `serve_pulls`: a removal applied in between
        // is seen here, before the frame goes out.
        ensure_listed(ctx, peer).await?;
        if let Some(message) = message {
            write_frame(&mut send, &message, FRAME_LIMIT).await?;
            last_sent = Some(own);
        }
        if changed.changed().await.is_err() {
            return Ok(());
        }
    }
}

/// Ends the session when the effective device list no longer names `peer`
/// with its endpoint, or removes it. A session is admitted once, at its
/// handshake; a removal this device learns of later has to reach it too.
async fn ensure_listed(ctx: &SessionContext, peer: &Peer) -> Result<(), SessionError> {
    let (replica, vault) = (Arc::clone(&ctx.replica), ctx.vault);
    let (device, endpoint) = (peer.device_pubkey, peer.endpoint_id);
    let listed = tokio::task::spawn_blocking(move || {
        crate::storage::query::read(replica.db(), |r| {
            let valid = device_list::valid_lists(&device_list::load_all(r)?, &vault);
            Ok(device_list::effective(&valid).is_some_and(|signed| {
                !signed.list.removes(&device)
                    && signed
                        .list
                        .device(&device)
                        .is_some_and(|entry| entry.endpoint_id == endpoint)
            }))
        })
    })
    .await??;
    if listed {
        Ok(())
    } else {
        Err(SessionError::NoLongerListed)
    }
}

/// What this device knows of when devices were last online, for the peer
/// (FR-033). Empty when it cannot be read; the next message carries it.
async fn last_seen_report(ctx: &SessionContext) -> Vec<([u8; 32], u64)> {
    let replica = Arc::clone(&ctx.replica);
    let own = ctx.keys.device_pubkey;
    tokio::task::spawn_blocking(move || {
        crate::storage::query::read(replica.db(), |r| seen::snapshot(r, &own, now_ms()))
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default()
}

/// Wall-clock milliseconds since the Unix epoch.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Pulls whenever the other side is further for some origin.
async fn pull_when_behind(
    ctx: &SessionContext,
    connection: &Connection,
    mut theirs: watch::Receiver<Vector>,
    peer: &Peer,
) -> Result<(), SessionError> {
    let mut changed = ctx.changed.clone();
    let mut pulled_at: Option<(Vector, Vector)> = None;
    loop {
        let their_vector = theirs.borrow_and_update().clone();
        changed.mark_unchanged();
        // Lowered for parked groups to fetch again ([`Replica::pull_vector`]).
        let (own, floors) = pull_vector(&ctx.replica).await?;
        // A pull that leaves this device's progress below theirs on purpose (a group not parked
        // at the parking limit, research R10) is not repeated until either side moved: the same
        // pages would otherwise come again at once, over and over.
        let at = (their_vector, own);
        if progress::has_more(&at.0, &at.1) && pulled_at.as_ref() != Some(&at) {
            pull(ctx, connection, at.1.clone(), floors).await?;
            // The pull may have brought a list that removes the peer.
            ensure_listed(ctx, peer).await?;
            pulled_at = Some(at);
            continue;
        }
        tokio::select! {
            result = theirs.changed() => if result.is_err() { return Ok(()) },
            result = changed.changed() => if result.is_err() { return Ok(()) },
        }
    }
}

/// One pull on its own stream. A `Resync` answer turns into a snapshot pull
/// right away (research R20).
async fn pull(
    ctx: &SessionContext,
    connection: &Connection,
    own: Vector,
    floors: Vec<Floor>,
) -> Result<(), SessionError> {
    if !pull_pages(ctx, connection, own, false, floors.clone()).await? {
        log::info!("sync: this device is too far behind, replacing from a snapshot");
        if !pull_pages(ctx, connection, Vector::new(), true, floors).await? {
            return Err(SessionError::Protocol("a resync answered a snapshot pull"));
        }
    }
    Ok(())
}

/// Sends one `Pull` and applies the pages it is answered with; `false` when
/// the answer is `Resync` instead. A snapshot (`replace`) also removes the
/// local rows it did not carry once its last page is applied. `floors` are the
/// parked groups `own` asks for again.
async fn pull_pages(
    ctx: &SessionContext,
    connection: &Connection,
    own: Vector,
    replace: bool,
    floors: Vec<Floor>,
) -> Result<bool, SessionError> {
    let (mut send, mut recv) = connection.open_bi().await?;
    let request = Message::Pull {
        vector: own,
        replace,
    };
    write_frame(&mut send, &request, FRAME_LIMIT).await?;
    send.finish()
        .map_err(|e| SessionError::Stream(e.to_string()))?;

    let mut inbox = if replace {
        Inbox::for_snapshot()
    } else {
        Inbox::new()
    }
    .refetching(floors);
    let mut tables = BTreeSet::new();
    loop {
        let page = match expect_frame(&mut recv, FRAME_LIMIT).await? {
            Message::Page(page) => page,
            Message::Resync { .. } if !replace => return Ok(false),
            _ => return Err(SessionError::Protocol("expected a Page")),
        };
        let replica = Arc::clone(&ctx.replica);
        let (back, received) = tokio::task::spawn_blocking(move || {
            let received = inbox.receive(&replica, page);
            (inbox, received)
        })
        .await?;
        inbox = back;
        let received = received?;
        tables.extend(received.tables);
        ctx.bump.send_modify(|n| *n = n.wrapping_add(1));
        if received.done {
            break;
        }
    }
    if let Some((kept, served)) = inbox.into_snapshot() {
        let replica = Arc::clone(&ctx.replica);
        let pruned = tokio::task::spawn_blocking(move || {
            resync::prune_absent_and_advance(&replica, &kept, &served)
        })
        .await??;
        tables.extend(pruned);
        ctx.bump.send_modify(|n| *n = n.wrapping_add(1));
    }
    if tables.contains("vault_key_envelopes") || tables.contains("device_lists") {
        unwrap_envelopes(ctx).await?;
    }
    if !tables.is_empty() {
        (ctx.on_applied)(tables);
    }
    Ok(true)
}

/// Serves the other side's pulls, one at a time.
async fn serve_pulls(
    ctx: &SessionContext,
    connection: &Connection,
    peer: &Peer,
) -> Result<(), SessionError> {
    loop {
        let (mut send, mut recv) = connection.accept_bi().await?;
        let Message::Pull { vector, replace } = expect_frame(&mut recv, FRAME_LIMIT).await? else {
            return Err(SessionError::Protocol("expected a Pull"));
        };
        let replica = Arc::clone(&ctx.replica);
        let served =
            tokio::task::spawn_blocking(move || serve(&replica, &vector, replace)).await??;
        // A session opened before the peer was removed must not go on serving it once this
        // device knows (FR-027). Checked after the data is read, not before: a removal and the
        // changes that follow it can be applied between the two, and only a check that comes
        // last sees every list the served data was read under.
        ensure_listed(ctx, peer).await?;
        let mut outbox = match served {
            Served::Pages(outbox) => outbox,
            Served::Resync => {
                let answer = Message::Resync {
                    reason: ResyncReason::TombstonesExpired,
                };
                write_frame(&mut send, &answer, FRAME_LIMIT).await?;
                send.finish()
                    .map_err(|e| SessionError::Stream(e.to_string()))?;
                continue;
            }
        };
        while let Some(page) = outbox.next_page() {
            write_frame(&mut send, &Message::Page(page), FRAME_LIMIT).await?;
        }
        send.finish()
            .map_err(|e| SessionError::Stream(e.to_string()))?;
    }
}

async fn pull_vector(replica: &Arc<Replica>) -> Result<(Vector, Vec<Floor>), SessionError> {
    let replica = Arc::clone(replica);
    Ok(tokio::task::spawn_blocking(move || replica.pull_vector()).await??)
}

async fn own_progress(replica: &Arc<Replica>) -> Result<Vector, SessionError> {
    let replica = Arc::clone(replica);
    Ok(tokio::task::spawn_blocking(move || replica.progress()).await??)
}

/// Opens content keys wrapped for this device that just arrived.
async fn unwrap_envelopes(ctx: &SessionContext) -> Result<(), SessionError> {
    let replica = Arc::clone(&ctx.replica);
    let keys = Arc::clone(&ctx.keys);
    let vault = ctx.vault;
    tokio::task::spawn_blocking(move || {
        replica.db().write(|tx| {
            let valid = device_list::valid_lists(&device_list::load_all(tx)?, &vault);
            content_keys::unwrap_own_envelopes(tx, &keys, &valid)
        })
    })
    .await??;
    Ok(())
}
