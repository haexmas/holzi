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
use crate::sync::inbound::Inbox;
use crate::sync::keys::DeviceKeys;
use crate::sync::outbound::serve_pull;
use crate::sync::progress::{self, Vector};
use crate::sync::replica::Replica;
use crate::sync::wire::{expect_frame, read_frame, write_frame, ErrorCode, Message, FRAME_LIMIT};

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
}

impl SessionError {
    fn code(&self) -> ErrorCode {
        match self {
            SessionError::Wire(e) => e.code(),
            SessionError::Inbound(_) => ErrorCode::PullFailed,
            SessionError::Protocol(_) => ErrorCode::Protocol,
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
    let (theirs_tx, theirs_rx) = watch::channel(Vector::new());
    let result = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => Ok(()),
        error = connection.closed() => {
            log::debug!("sync: the connection ended: {error}");
            Ok(())
        }
        result = read_progress(control_recv, theirs_tx) => result,
        result = send_progress(&ctx, control_send) => result,
        result = pull_when_behind(&ctx, &connection, theirs_rx) => result,
        result = serve_pulls(&ctx, &connection) => result,
    };
    match result {
        Ok(()) => connection.close(ErrorCode::Closed.as_u32().into(), b"closed"),
        Err(error) => {
            log::warn!(
                "sync: the session with {} ended: {error}",
                crate::sync::keys::hex(&peer.device_pubkey[..4])
            );
            connection.close(error.code().as_u32().into(), b"error");
        }
    }
}

/// Keeps the other side's latest progress.
async fn read_progress(
    mut recv: RecvStream,
    theirs: watch::Sender<Vector>,
) -> Result<(), SessionError> {
    while let Some(message) = read_frame(&mut recv, FRAME_LIMIT).await? {
        match message {
            Message::Progress { vector, .. } => {
                theirs.send_replace(vector);
            }
            // Pushes after the handshake are for user story 2.
            Message::DeviceListPush { .. } => {}
            _ => return Err(SessionError::Protocol("an unexpected control message")),
        }
    }
    Ok(())
}

/// Sends this device's progress at the start and whenever it moved.
async fn send_progress(ctx: &SessionContext, mut send: SendStream) -> Result<(), SessionError> {
    let mut changed = ctx.changed.clone();
    let mut last_sent: Option<Vector> = None;
    loop {
        changed.mark_unchanged();
        let own = own_progress(&ctx.replica).await?;
        if last_sent.as_ref() != Some(&own) {
            let message = Message::Progress {
                vector: own.clone(),
                last_seen: Vec::new(),
            };
            write_frame(&mut send, &message, FRAME_LIMIT).await?;
            last_sent = Some(own);
        }
        if changed.changed().await.is_err() {
            return Ok(());
        }
    }
}

/// Pulls whenever the other side is further for some origin.
async fn pull_when_behind(
    ctx: &SessionContext,
    connection: &Connection,
    mut theirs: watch::Receiver<Vector>,
) -> Result<(), SessionError> {
    let mut changed = ctx.changed.clone();
    loop {
        let their_vector = theirs.borrow_and_update().clone();
        changed.mark_unchanged();
        let own = own_progress(&ctx.replica).await?;
        if progress::has_more(&their_vector, &own) {
            pull(ctx, connection, own).await?;
            continue;
        }
        tokio::select! {
            result = theirs.changed() => if result.is_err() { return Ok(()) },
            result = changed.changed() => if result.is_err() { return Ok(()) },
        }
    }
}

/// One pull on its own stream.
async fn pull(
    ctx: &SessionContext,
    connection: &Connection,
    own: Vector,
) -> Result<(), SessionError> {
    let (mut send, mut recv) = connection.open_bi().await?;
    let request = Message::Pull {
        vector: own,
        replace: false,
    };
    write_frame(&mut send, &request, FRAME_LIMIT).await?;
    send.finish()
        .map_err(|e| SessionError::Stream(e.to_string()))?;

    let mut inbox = Inbox::new();
    let mut tables = BTreeSet::new();
    loop {
        let page = match expect_frame(&mut recv, FRAME_LIMIT).await? {
            Message::Page(page) => page,
            Message::Resync { reason } => {
                // Replacing the synced tables follows with user story 3.
                log::warn!("sync: the other device asks for a resync ({reason:?})");
                return Ok(());
            }
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
    if tables.contains("vault_key_envelopes") || tables.contains("device_lists") {
        unwrap_envelopes(ctx).await?;
    }
    if !tables.is_empty() {
        (ctx.on_applied)(tables);
    }
    Ok(())
}

/// Serves the other side's pulls, one at a time.
async fn serve_pulls(ctx: &SessionContext, connection: &Connection) -> Result<(), SessionError> {
    loop {
        let (mut send, mut recv) = connection.accept_bi().await?;
        let Message::Pull { vector, replace } = expect_frame(&mut recv, FRAME_LIMIT).await? else {
            return Err(SessionError::Protocol("expected a Pull"));
        };
        if replace {
            // A snapshot for replacing follows with user story 3.
            return Err(SessionError::Protocol("replace is not supported yet"));
        }
        let replica = Arc::clone(&ctx.replica);
        let mut outbox =
            tokio::task::spawn_blocking(move || serve_pull(&replica, &vector)).await??;
        while let Some(page) = outbox.next_page() {
            write_frame(&mut send, &Message::Page(page), FRAME_LIMIT).await?;
        }
        send.finish()
            .map_err(|e| SessionError::Stream(e.to_string()))?;
    }
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
