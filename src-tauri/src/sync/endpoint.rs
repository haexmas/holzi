//! The iroh endpoint of a vault session (spec 024, FR-009, FR-031,
//! research R6).
//!
//! One endpoint per session, built after unlocking with the stored endpoint
//! key and `presets::Minimal`: no n0 address publishing; addresses come
//! from presence (user story 1, step 5) into a `MemoryLookup`. A `Router`
//! accepts `holzi-sync/1`; the accepting side opens the handshake stream.
//! Per device at most one connection lives: when both dial each other at
//! once, the connection dialed by the smaller endpoint id stays.

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::address_lookup::MemoryLookup;
use iroh::endpoint::{presets, Connection};
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::{Endpoint, EndpointAddr, RelayConfig, RelayMode, RelayUrl, SecretKey};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::sync::handshake::{self, local_schema, Local, Peer};
use crate::sync::keys::DeviceKeys;
use crate::sync::problems::{DuplicateWatch, Problem};
use crate::sync::replica::Replica;
use crate::sync::session::{self, SessionContext};
use crate::sync::wire::{ErrorCode, RejectCode, SYNC_ALPN};

/// How long binding may take; it can hang on resolving iroh-Relays.
const BIND_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a refusing side waits for the refused peer to read the refusal and close, before
/// it closes itself.
const REFUSAL_GRACE: Duration = Duration::from_secs(2);
/// How long the router may take to shut down at the session end.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// How the endpoint reaches other devices.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub relay_mode: RelayMode,
    /// Bind only this address instead of every interface; tests use
    /// loopback.
    pub bind_addr: Option<SocketAddr>,
}

/// Why the node could not start or connect.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("binding the endpoint failed: {0}")]
    Bind(String),
    #[error("binding the endpoint timed out")]
    BindTimeout,
    #[error("connecting failed: {0}")]
    Connect(String),
}

/// A running endpoint with its sessions.
pub struct SyncNode {
    inner: Arc<Inner>,
    router: Router,
}

struct Inner {
    endpoint: Endpoint,
    lookup: MemoryLookup,
    replica: Arc<Replica>,
    keys: Arc<DeviceKeys>,
    vault: [u8; 32],
    changed: watch::Receiver<u64>,
    bump: Arc<watch::Sender<u64>>,
    on_applied: Arc<dyn Fn(BTreeSet<String>) + Send + Sync>,
    cancel: CancellationToken,
    tracker: TaskTracker,
    /// Live connections per device key.
    peers: Mutex<HashMap<[u8; 32], Connection>>,
    /// The relay URLs last applied to `endpoint` (spec 024, FR-008):
    /// `Endpoint` has no getter for its own relay set, so this is the
    /// only record of it, seeded from the bind-time `RelayMode`.
    applied_relays: Mutex<Vec<RelayUrl>>,
    /// Devices seen as duplicates lately; sync with them stays halted.
    duplicates: DuplicateWatch,
    /// Called when a device's problem was set or cleared (FR-034).
    devices_changed: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    /// Devices this device's list does not name whose presence claimed a
    /// newer list, waiting to be dialed once to fetch it (FR-007).
    candidates: Mutex<HashMap<[u8; 32], EndpointAddr>>,
}

/// Most devices kept as candidates at once, so a flood of meetings cannot
/// make this device dial without end.
const MAX_CANDIDATES: usize = 16;

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncNode")
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
struct SyncProtocol(Arc<Inner>);

impl ProtocolHandler for SyncProtocol {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        // Tracked, not just awaited inline: `Router::shutdown` aborts this
        // future right after the (default, no-op) `ProtocolHandler::shutdown`
        // resolves, with no grace period. Spawning onto `inner.tracker` moves
        // the actual session work to a task that keeps running independently
        // and that `SyncNode::shutdown`'s own `tracker.wait()` waits for, so
        // it can still observe `cancel` and close the connection cleanly.
        let inner = Arc::clone(&self.0);
        inner
            .tracker
            .spawn(run_connection(Arc::clone(&inner), connection, Side::Accept));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Accept,
    Dial,
}

impl SyncNode {
    /// Binds the endpoint and starts accepting.
    pub async fn bind(
        replica: Arc<Replica>,
        keys: DeviceKeys,
        vault: [u8; 32],
        config: NodeConfig,
        on_applied: Arc<dyn Fn(BTreeSet<String>) + Send + Sync>,
    ) -> Result<Self, NodeError> {
        let lookup = MemoryLookup::new();
        let initial_relays = config.relay_mode.relay_map().urls::<Vec<RelayUrl>>();
        let mut builder = Endpoint::builder(presets::Minimal)
            .secret_key(SecretKey::from_bytes(&keys.endpoint_secret))
            .alpns(vec![SYNC_ALPN.to_vec()])
            .relay_mode(config.relay_mode)
            .address_lookup(lookup.clone());
        if let Some(addr) = config.bind_addr {
            builder = builder
                .clear_ip_transports()
                .bind_addr(addr)
                .map_err(|e| NodeError::Bind(e.to_string()))?;
        }
        let endpoint = tokio::time::timeout(BIND_TIMEOUT, builder.bind())
            .await
            .map_err(|_| NodeError::BindTimeout)?
            .map_err(|e| NodeError::Bind(e.to_string()))?;

        let (bump, changed) = watch::channel(0u64);
        let inner = Arc::new(Inner {
            endpoint: endpoint.clone(),
            lookup,
            replica,
            keys: Arc::new(keys),
            vault,
            changed,
            bump: Arc::new(bump),
            on_applied,
            cancel: CancellationToken::new(),
            tracker: TaskTracker::new(),
            peers: Mutex::new(HashMap::new()),
            applied_relays: Mutex::new(initial_relays),
            duplicates: DuplicateWatch::default(),
            devices_changed: Mutex::new(None),
            candidates: Mutex::new(HashMap::new()),
        });
        let router = Router::builder(endpoint)
            .accept(SYNC_ALPN, SyncProtocol(Arc::clone(&inner)))
            .spawn();
        Ok(Self { inner, router })
    }

    /// This endpoint's address, for presence and tests.
    pub fn addr(&self) -> EndpointAddr {
        self.inner.endpoint.addr()
    }

    /// Dials a device at `addr` and runs the session in the background.
    pub async fn connect(&self, addr: EndpointAddr) -> Result<(), NodeError> {
        self.inner.lookup.add_endpoint_info(addr.clone());
        let connection = self
            .inner
            .endpoint
            .connect(addr, SYNC_ALPN)
            .await
            .map_err(|e| NodeError::Connect(e.to_string()))?;
        let inner = Arc::clone(&self.inner);
        self.inner
            .tracker
            .spawn(run_connection(inner, connection, Side::Dial));
        Ok(())
    }

    /// Dials `addr` on the link protocol and hands the connection to the
    /// caller, which runs the exchange itself (user story 5: the main device
    /// dials the new installation).
    pub async fn dial_link(&self, addr: EndpointAddr) -> Result<Connection, NodeError> {
        self.inner.lookup.add_endpoint_info(addr.clone());
        self.inner
            .endpoint
            .connect(addr, crate::sync::wire::LINK_ALPN)
            .await
            .map_err(|e| NodeError::Connect(e.to_string()))
    }

    /// Tells every session that this device's progress may have moved, as
    /// after a local commit. Close signals collapse into one.
    pub fn local_changed(&self) {
        self.inner.bump.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// This device's own key, which is on its device list but never a peer.
    pub fn device_pubkey(&self) -> [u8; 32] {
        self.inner.keys.device_pubkey
    }

    /// Sets what runs when a device's problem appears or goes away.
    pub fn on_devices_changed(&self, hook: Arc<dyn Fn() + Send + Sync>) {
        let mut slot = self
            .inner
            .devices_changed
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *slot = Some(hook);
    }

    /// Halts sync with `device` on evidence of `problem` (FR-029, FR-030):
    /// records it, and for a duplicate also holds the device back and ends
    /// its live session, since the real one cannot be told from the copy.
    pub async fn flag(&self, device: [u8; 32], problem: Problem) {
        flag(&self.inner, device, problem).await;
    }

    /// Lets go of a device that was removed from the vault (FR-027): ends
    /// its live session and forgets where it could be reached, so nothing
    /// dials it again. A connection it opens later fails the handshake.
    pub fn forget(&self, device: [u8; 32], endpoint_id: [u8; 32]) {
        let live = self
            .inner
            .peers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&device);
        if let Some(live) = live {
            live.close(ErrorCode::Rejected.as_u32().into(), b"removed");
        }
        if let Ok(id) = iroh::EndpointId::from_bytes(&endpoint_id) {
            self.inner.lookup.remove_endpoint_info(id);
        }
    }

    /// Tells the interface the device list or a device's state changed,
    /// for changes made outside the node (a link that just published).
    pub fn announce_devices_changed(&self) {
        notify_devices_changed(&self.inner);
    }

    /// Devices with a live session.
    pub fn connected(&self) -> Vec<[u8; 32]> {
        let peers = self.inner.peers.lock().unwrap_or_else(|e| e.into_inner());
        peers.keys().copied().collect()
    }

    /// Records a device's address from presence (spec 024, FR-007), so
    /// dialing it later can find it. Does not dial by itself; reconnect
    /// (FR-010) decides when to.
    pub async fn note_presence(&self, addr: EndpointAddr) {
        self.inner.lookup.add_endpoint_info(addr);
    }

    /// Keeps the address of a device that is not on this device's list but
    /// announced a newer list (a copy of a main device enrolled itself, FR-007),
    /// for one dial that fetches the list. `false` when too many wait already.
    pub fn note_candidate(&self, device: [u8; 32], addr: EndpointAddr) -> bool {
        let mut candidates = self
            .inner
            .candidates
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if candidates.len() >= MAX_CANDIDATES && !candidates.contains_key(&device) {
            return false;
        }
        self.inner.lookup.add_endpoint_info(addr.clone());
        candidates.insert(device, addr);
        true
    }

    /// The candidates to dial now; each is dialed once per announcement.
    pub fn take_candidates(&self) -> Vec<EndpointAddr> {
        let mut candidates = self
            .inner
            .candidates
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        candidates.drain().map(|(_, addr)| addr).collect()
    }

    /// Brings the endpoint's relay set in line with `mode` (spec 024,
    /// FR-008: relay changes from the settings apply at runtime). Takes the
    /// same [`RelayMode`] as [`Self::bind`] (from
    /// [`crate::sync::servers::ServerConfig::relay_mode`]), so disabling every
    /// configured relay produces a relay-free endpoint instead of restoring
    /// the defaults.
    pub async fn apply_relays(&self, mode: &RelayMode) {
        let want = mode.relay_map().urls::<Vec<RelayUrl>>();
        let have = {
            let applied = self
                .inner
                .applied_relays
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            applied.clone()
        };
        let (to_insert, to_remove) = crate::sync::servers::diff_relays(&have, &want);
        for url in &to_remove {
            self.inner.endpoint.remove_relay(url).await;
        }
        for url in &to_insert {
            self.inner
                .endpoint
                .insert_relay(url.clone(), Arc::new(RelayConfig::from(url.clone())))
                .await;
        }
        if !to_insert.is_empty() || !to_remove.is_empty() {
            let mut applied = self
                .inner
                .applied_relays
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *applied = want;
        }
    }

    /// Ends every session and closes the endpoint (FR-031).
    pub async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tracker.close();
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, self.router.shutdown())
            .await
            .is_err()
        {
            log::warn!("sync: the endpoint did not shut down within {SHUTDOWN_TIMEOUT:?}");
        }
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, self.inner.tracker.wait())
            .await
            .is_err()
        {
            log::warn!(
                "sync: {SHUTDOWN_TIMEOUT:?} was not enough for every session to end; \
                 some may keep running in the background"
            );
        }
    }
}

/// Handshake, then the session, for one connection.
async fn run_connection(inner: Arc<Inner>, connection: Connection, side: Side) {
    let remote = *connection.remote_id().as_bytes();
    let streams = match side {
        Side::Accept => connection.open_bi().await,
        Side::Dial => connection.accept_bi().await,
    };
    let (mut send, mut recv) = match streams {
        Ok(streams) => streams,
        Err(error) => {
            log::debug!("sync: no handshake stream: {error}");
            return;
        }
    };
    let local = Local {
        keys: &inner.keys,
        vault: inner.vault,
        schema: local_schema(),
    };
    let handshake = match side {
        Side::Accept => {
            handshake::accept(&mut send, &mut recv, &inner.replica, &local, remote).await
        }
        Side::Dial => handshake::dial(&mut send, &mut recv, &inner.replica, &local, remote).await,
    };
    let peer = match handshake {
        Ok(peer) => peer,
        Err(error) => {
            log::info!("sync: handshake failed: {error}");
            note_failure(&inner, &error, remote).await;
            let _ = send.finish();
            if side == Side::Accept {
                // What this side wrote last — the lists it pushed and the refusal — must reach
                // the peer before the connection closes, or the peer sees only "connection
                // lost" and never learns, for example, that it was removed (FR-034). The peer
                // closes when it has read it.
                let _ = tokio::time::timeout(REFUSAL_GRACE, connection.closed()).await;
            }
            connection.close(handshake_close_code(&error).as_u32().into(), b"rejected");
            return;
        }
    };
    if inner.duplicates.holds(&peer.device_pubkey) {
        connection.close(ErrorCode::Rejected.as_u32().into(), b"duplicate");
        return;
    }
    clear_problem(&inner, peer.device_pubkey).await;
    if !register(&inner, &peer, &connection, side, remote) {
        connection.close(ErrorCode::Closed.as_u32().into(), b"duplicate");
        return;
    }
    // The device is online now: its time is now, and the list shows it.
    mark_seen(&inner, peer.device_pubkey).await;
    notify_devices_changed(&inner);

    let ctx = SessionContext {
        replica: Arc::clone(&inner.replica),
        keys: Arc::clone(&inner.keys),
        vault: inner.vault,
        changed: inner.changed.clone(),
        bump: Arc::clone(&inner.bump),
        on_applied: Arc::clone(&inner.on_applied),
        on_devices_changed: {
            let inner = Arc::clone(&inner);
            Arc::new(move || notify_devices_changed(&inner))
        },
        cancel: inner.cancel.child_token(),
    };
    session::run(ctx, connection.clone(), send, recv, peer.clone()).await;

    let was_live = {
        let mut peers = inner.peers.lock().unwrap_or_else(|e| e.into_inner());
        let live = peers
            .get(&peer.device_pubkey)
            .is_some_and(|live| live.stable_id() == connection.stable_id());
        if live {
            peers.remove(&peer.device_pubkey);
        }
        live
    };
    if was_live {
        // The last moment it was online is when the session ended.
        mark_seen(&inner, peer.device_pubkey).await;
        notify_devices_changed(&inner);
    }
}

/// The code to close the connection with: a wire error keeps its own
/// (already distinct) code, everything else collapses to one of the two
/// handshake-level codes.
fn handshake_close_code(error: &handshake::HandshakeError) -> ErrorCode {
    match error {
        handshake::HandshakeError::Wire(e) => e.code(),
        handshake::HandshakeError::Refused(_)
        | handshake::HandshakeError::RefusedByPeer(_)
        | handshake::HandshakeError::Halted { .. } => ErrorCode::Rejected,
        handshake::HandshakeError::Protocol(_)
        | handshake::HandshakeError::NoDeviceList
        | handshake::HandshakeError::Crdt(_)
        | handshake::HandshakeError::Join(_)
        | handshake::HandshakeError::Signing(_) => ErrorCode::Protocol,
    }
}

/// Keeps one connection per device: the one dialed by the smaller endpoint
/// id. Returns whether `connection` is the one to keep.
fn register(
    inner: &Inner,
    peer: &Peer,
    connection: &Connection,
    side: Side,
    remote: [u8; 32],
) -> bool {
    let own = inner.keys.endpoint_id;
    let dialer = if side == Side::Dial { own } else { remote };
    let preferred = dialer == own.min(remote);
    let mut peers = inner.peers.lock().unwrap_or_else(|e| e.into_inner());
    match peers.get(&peer.device_pubkey) {
        Some(existing) if existing.close_reason().is_none() && !preferred => false,
        Some(existing) => {
            if existing.stable_id() != connection.stable_id() {
                existing.close(ErrorCode::Closed.as_u32().into(), b"duplicate");
            }
            peers.insert(peer.device_pubkey, connection.clone());
            true
        }
        None => {
            peers.insert(peer.device_pubkey, connection.clone());
            true
        }
    }
}

#[path = "endpoint_devices.rs"]
mod devices;
use devices::{clear_problem, flag, mark_seen, note_failure, notify_devices_changed};

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
