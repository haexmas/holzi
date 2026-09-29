//! Presence over Nostr, so devices of the same vault find each other and
//! connect on their own (spec 024, FR-007, FR-008, FR-010,
//! contracts/nostr-events.md).
//!
//! A presence meeting follows NIP-59's three layers, but with a shared
//! *mailbox* key pair instead of one recipient's real identity, so every
//! device that holds the vault's current content key can open it, and with
//! real (not randomized-backward) timestamps, since these events are
//! ephemeral (kind 21059) and never stored by a relay:
//!
//! - gift wrap (kind 21059): signed by a fresh, one-time key; `p`-tag the
//!   mailbox pubkey `mb_pk`; content = NIP-44 v2 of the seal, encrypted to
//!   `mb_pk`.
//! - seal (kind 13): signed by the sender's real device key; content =
//!   NIP-44 v2 of the inner event, *also* encrypted to `mb_pk` (not the
//!   sender-recipient pair NIP-59 normally uses) — the ECDH shared secret is
//!   the same computed from either side (`ECDH(sender_sk, mb_pk) ==
//!   ECDH(mb_sk, sender_pk)`), so any device that can derive `mb_sk` can
//!   open it, regardless of which specific device sent it.
//! - inner event (kind 24100, unsigned): its own content is the presence
//!   JSON below.
//!
//! `mb_pk`/`mb_sk` are derived from the vault's current content key and
//! today's (or yesterday's) date, so they rotate daily without any of the
//! devices agreeing on it out of band.

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use hkdf::Hkdf;
use iroh::{EndpointAddr, RelayUrl, TransportAddr};
use nostr::event::{
    Event, EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, Tag, UnsignedEvent,
};
use nostr::key::{Keys, PublicKey, SecretKey};
use nostr::nips::nip44;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::storage::query::Query;
use crate::sync::keys::{random_bytes, DeviceKeys};

/// A presence meeting is never sent or kept over this size (contracts/
/// nostr-events.md).
const MAX_EVENT_BYTES: usize = 16 * 1024;
/// How stale a meeting's `ts` may be.
const MAX_AGE_MS: u64 = 150_000;
/// How far into the future a meeting's `ts` may claim to be.
const MAX_FUTURE_MS: u64 = 30_000;
/// The presence event kind (contracts/nostr-events.md).
const PRESENCE_KIND: Kind = Kind::Custom(24100);
/// The gift-wrap kind holzi uses (not NIP-59's standard 1059: presence is
/// ephemeral and never stored, so it uses the ephemeral range instead).
const GIFT_WRAP_KIND: Kind = Kind::Custom(21059);

#[derive(Debug, thiserror::Error)]
pub enum PresenceError {
    #[error("the event is over the {MAX_EVENT_BYTES}-byte limit")]
    TooLarge,
    #[error("not a valid secp256k1 key")]
    InvalidKey,
    #[error("the gift wrap or seal does not decrypt or verify: {0}")]
    Crypto(String),
    #[error("the payload does not decode: {0}")]
    Malformed(String),
    #[error("the inner event is not a presence meeting")]
    WrongKind,
}

impl From<PresenceError> for haex_crdt::Error {
    /// Carries a presence failure through a storage transaction as a consumer error.
    fn from(error: PresenceError) -> Self {
        haex_crdt::Error::consumer(error.to_string())
    }
}

/// The presence content (contracts/nostr-events.md): this device's current
/// reachability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceContent {
    pub v: u8,
    #[serde(with = "hex_bytes32")]
    pub device: [u8; 32],
    #[serde(with = "hex_bytes32")]
    pub endpoint: [u8; 32],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iroh_relay: Option<String>,
    pub addrs: Vec<String>,
    pub list_generation: u64,
    pub ts: u64,
    #[serde(with = "hex_nonce")]
    pub nonce: [u8; 16],
}

impl PresenceContent {
    /// Builds this device's own current presence content.
    pub fn own(
        device_pubkey: [u8; 32],
        endpoint_id: [u8; 32],
        iroh_relay: Option<String>,
        addrs: Vec<SocketAddr>,
        list_generation: u64,
    ) -> Self {
        Self {
            v: 1,
            device: device_pubkey,
            endpoint: endpoint_id,
            iroh_relay,
            addrs: addrs.iter().map(SocketAddr::to_string).collect(),
            list_generation,
            ts: now_ms(),
            nonce: *random_bytes::<16>(),
        }
    }

    /// Whether `ts` is fresh enough to act on (contracts/nostr-events.md).
    pub fn is_fresh(&self, now_ms: u64) -> bool {
        let age = now_ms.saturating_sub(self.ts);
        let future = self.ts.saturating_sub(now_ms);
        age <= MAX_AGE_MS && future <= MAX_FUTURE_MS
    }

    /// The [`EndpointAddr`] this meeting names, for [`iroh::address_lookup::MemoryLookup`].
    pub fn endpoint_addr(&self) -> Result<EndpointAddr, PresenceError> {
        let id =
            iroh::EndpointId::from_bytes(&self.endpoint).map_err(|_| PresenceError::InvalidKey)?;
        let mut addrs: Vec<TransportAddr> = self
            .addrs
            .iter()
            .filter_map(|a| a.parse::<SocketAddr>().ok())
            .map(TransportAddr::Ip)
            .collect();
        if let Some(relay) = &self.iroh_relay {
            if let Ok(url) = relay.parse::<RelayUrl>() {
                addrs.push(TransportAddr::Relay(url));
            }
        }
        Ok(EndpointAddr::from_parts(id, addrs))
    }
}

/// Wall-clock milliseconds since the Unix epoch; 0 if the clock is before it.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Days since 1970-01-01 UTC, for today's and yesterday's mailbox tag.
pub fn day_tag_now() -> u32 {
    day_tag_at(now_ms())
}

/// Days since 1970-01-01 UTC at `now_ms`, the mailbox rotation step.
fn day_tag_at(now_ms: u64) -> u32 {
    (now_ms / 86_400_000) as u32
}

/// The mailbox key pair for `day_tag`, derived from the vault's content key
/// (contracts/nostr-events.md): `HKDF-SHA256(ikm = content_key, info =
/// "holzi/presence/v1" ‖ tag_u32 BE)`.
pub fn mailbox_keys(
    content_key: &[u8; 32],
    day_tag: u32,
) -> Result<(SecretKey, PublicKey), PresenceError> {
    let hkdf = Hkdf::<Sha256>::new(None, content_key);
    let mut info = b"holzi/presence/v1".to_vec();
    info.extend_from_slice(&day_tag.to_be_bytes());
    let mut okm = [0u8; 32];
    hkdf.expand(&info, &mut okm)
        .expect("32 bytes are a valid HKDF-SHA256 output length");
    let secret = SecretKey::from_slice(&okm).map_err(|_| PresenceError::InvalidKey)?;
    let public = Keys::new(secret.clone()).public_key();
    Ok((secret, public))
}

/// A raw device secret as a Nostr secret key.
fn nostr_secret(secret: &[u8; 32]) -> Result<SecretKey, PresenceError> {
    SecretKey::from_slice(secret).map_err(|_| PresenceError::InvalidKey)
}

/// A raw x-only pubkey as a Nostr public key.
fn nostr_public(pubkey: &[u8; 32]) -> Result<PublicKey, PresenceError> {
    PublicKey::from_slice(pubkey).map_err(|_| PresenceError::InvalidKey)
}

/// Builds the gift-wrapped presence meeting ready to publish.
pub fn build(
    sender: &DeviceKeys,
    content: &PresenceContent,
    mb_pk: &PublicKey,
) -> Result<Event, PresenceError> {
    let json =
        serde_json::to_string(content).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    wrap_payload(PRESENCE_KIND, &sender.device_secret, &json, mb_pk)
}

/// Opens a received gift-wrapped meeting: decrypts both layers, verifies the
/// seal's signature, and returns the authenticated sender device pubkey
/// (contracts/nostr-events.md — the caller still has to check it against the
/// effective device list) together with the presence content.
pub fn open(
    gift_wrap: &Event,
    mb_sk: &SecretKey,
) -> Result<([u8; 32], PresenceContent), PresenceError> {
    let (sender, json) = unwrap_payload(gift_wrap, mb_sk, PRESENCE_KIND)?;
    let content: PresenceContent =
        serde_json::from_str(&json).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    Ok((sender, content))
}

/// The three layers of a meeting (gift wrap, seal, unsigned inner event of
/// `inner_kind` carrying `payload`), sealed by `sender_secret` for the shared
/// mailbox key `recipient`. Presence and the link rendezvous use the same
/// layers with different inner kinds (contracts/nostr-events.md).
pub(crate) fn wrap_payload(
    inner_kind: Kind,
    sender_secret: &[u8; 32],
    payload: &str,
    recipient: &PublicKey,
) -> Result<Event, PresenceError> {
    let sender_secret = nostr_secret(sender_secret)?;
    let sender_keys = Keys::new(sender_secret.clone());

    let rumor: UnsignedEvent =
        EventBuilder::new(inner_kind, payload).finalize_unsigned(sender_keys.public_key());
    let rumor_json =
        serde_json::to_string(&rumor).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    let sealed = nip44::encrypt(
        &sender_secret,
        recipient,
        rumor_json.as_bytes(),
        nip44::Version::V2,
    )
    .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    let seal: Event = EventBuilder::new(Kind::Seal, sealed)
        .finalize(&sender_keys)
        .map_err(|e| PresenceError::Crypto(e.to_string()))?;

    let seal_json =
        serde_json::to_string(&seal).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    let onetime = Keys::generate();
    let wrapped = nip44::encrypt(
        onetime.secret_key(),
        recipient,
        seal_json.as_bytes(),
        nip44::Version::V2,
    )
    .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    EventBuilder::new(GIFT_WRAP_KIND, wrapped)
        .tag(Tag::public_key(*recipient))
        .finalize(&onetime)
        .map_err(|e| PresenceError::Crypto(e.to_string()))
}

/// Opens a gift wrap made by [`wrap_payload`]: decrypts both layers with
/// `mb_sk`, verifies the seal's signature and that the inner event has
/// `inner_kind`, and returns the seal signer with the inner payload.
pub(crate) fn unwrap_payload(
    gift_wrap: &Event,
    mb_sk: &SecretKey,
    inner_kind: Kind,
) -> Result<([u8; 32], String), PresenceError> {
    if gift_wrap.as_json().len() > MAX_EVENT_BYTES {
        return Err(PresenceError::TooLarge);
    }
    let seal_json = nip44::decrypt_to_bytes(mb_sk, &gift_wrap.pubkey, &gift_wrap.content)
        .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    let seal: Event =
        serde_json::from_slice(&seal_json).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    seal.verify()
        .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    if seal.kind != Kind::Seal {
        return Err(PresenceError::WrongKind);
    }

    let rumor_json = nip44::decrypt_to_bytes(mb_sk, &seal.pubkey, &seal.content)
        .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    let rumor: UnsignedEvent =
        serde_json::from_slice(&rumor_json).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    if rumor.kind != inner_kind {
        return Err(PresenceError::WrongKind);
    }
    Ok((seal.pubkey.to_bytes(), rumor.content))
}

/// Checks a device pubkey is a real secp256k1 key, for callers that only
/// have a raw `[u8; 32]` (e.g. from the device list) and need a
/// [`PublicKey`] to build a filter or verify against.
pub fn public_key(pubkey: &[u8; 32]) -> Result<PublicKey, PresenceError> {
    nostr_public(pubkey)
}

/// Records that `device_pubkey` was seen just now, reachable at
/// `endpoint_addr` (or with no address, if the meeting named none this
/// device could parse). An earlier `problem` stays: a device that is
/// reachable can still be one this device must not sync with, and only a
/// successful handshake clears it.
pub fn record_seen(
    tx: &mut CrdtTransaction<'_>,
    device_pubkey: &[u8; 32],
    last_seen_ms: u64,
    endpoint_addr: Option<&EndpointAddr>,
) -> haex_crdt::Result<()> {
    let encoded = endpoint_addr
        .map(postcard::to_stdvec)
        .transpose()
        .map_err(|e| haex_crdt::Error::consumer(format!("endpoint addr: {e}")))?;
    let last_seen = i64::try_from(last_seen_ms)
        .map_err(|_| haex_crdt::Error::consumer("last_seen beyond i64"))?;
    tx.execute(
        "INSERT INTO device_presence_no_sync (device_pubkey, last_seen, endpoint_addr, problem) \
         VALUES (?1, ?2, ?3, NULL) \
         ON CONFLICT(device_pubkey) DO UPDATE SET \
           last_seen = excluded.last_seen, endpoint_addr = excluded.endpoint_addr",
        params![device_pubkey.as_slice(), last_seen, encoded],
    )?;
    Ok(())
}

/// One device's presence row, as stored.
pub struct StoredPresence {
    pub device_pubkey: [u8; 32],
    pub last_seen_ms: u64,
    pub endpoint_addr: Option<EndpointAddr>,
}

/// Every device this device has ever recorded presence for.
pub fn load_all(q: &mut impl Query) -> haex_crdt::Result<Vec<StoredPresence>> {
    let rows: Vec<(Vec<u8>, i64, Option<Vec<u8>>)> = q.query_map(
        "SELECT device_pubkey, last_seen, endpoint_addr FROM device_presence_no_sync",
        &[],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    rows.into_iter()
        .map(|(device_pubkey, last_seen, endpoint_addr)| {
            let device_pubkey: [u8; 32] = device_pubkey
                .try_into()
                .map_err(|_| haex_crdt::Error::consumer("device_presence: malformed pubkey"))?;
            let endpoint_addr = endpoint_addr
                .map(|bytes| postcard::from_bytes(&bytes))
                .transpose()
                .map_err(|e| haex_crdt::Error::consumer(format!("endpoint addr: {e}")))?;
            Ok(StoredPresence {
                device_pubkey,
                last_seen_ms: u64::try_from(last_seen).unwrap_or(0),
                endpoint_addr,
            })
        })
        .collect()
}

/// How often this device republishes its own presence, absent an address
/// change (contracts/nostr-events.md).
const PUBLISH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
/// How long presence waits for its relays before it goes on without them
/// (Constitution VII: a slow or dead relay never blocks the others). The
/// relays keep connecting, and reconnecting, in the background.
const RELAY_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Installs `ring` as the process-wide rustls crypto provider, unless one is
/// already installed. The relay client's `wss://` websocket asks rustls for
/// that default, and with both `ring` (iroh) and `aws-lc-rs` (reqwest) in
/// the dependency tree rustls cannot choose on its own and panics instead.
pub fn ensure_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        // A concurrent install winning the race is just as good.
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

/// Runs presence for as long as this device's session lasts: connects to
/// `relay_urls`, subscribes to today's and yesterday's mailbox, publishes
/// this device's own reachability on the contract's cadence — and also
/// right away whenever `changed` fires, so a device list this session just
/// issued (linking, removing) does not wait out the rest of the 60 s tick
/// — and feeds every fresh, authenticated meeting it receives into `node`'s
/// address book and `device_presence_no_sync`, then wakes `reconnect` so
/// the caller's reconnect loop dials it (dialing here would stall presence
/// for as long as an offline device's dial takes). Returns only if the relay
/// client itself ends; the caller races this against its own cancellation.
pub async fn run(
    node: &crate::sync::endpoint::SyncNode,
    replica: &crate::sync::replica::Replica,
    keys: &DeviceKeys,
    vault: [u8; 32],
    relay_urls: Vec<String>,
    mut changed: tokio::sync::watch::Receiver<u64>,
    reconnect: &tokio::sync::Notify,
) {
    use futures::StreamExt;

    ensure_crypto_provider();
    let client = nostr_sdk::client::Client::new();
    for url in &relay_urls {
        if let Err(error) = client.add_relay(url.as_str()).await {
            log::warn!("sync: presence relay {url} is not a valid URL: {error}");
        }
    }
    // Unlike `try_connect_relay`, `connect` keeps a relay that is down right
    // now (e.g. the device opened the vault offline) reconnecting in the
    // background, and each reconnect renews the subscription (FR-010).
    client.connect().and_wait(RELAY_CONNECT_TIMEOUT).await;

    let mut notifications = client.notifications();
    let mut publish_tick = tokio::time::interval(PUBLISH_INTERVAL);
    let mut subscribed_day: Option<u32> = None;

    loop {
        let day = day_tag_now();
        if subscribed_day != Some(day) {
            if let Err(error) = resubscribe(&client, replica, vault, day).await {
                log::warn!("sync: presence subscription failed, retrying next tick: {error}");
            } else {
                subscribed_day = Some(day);
            }
        }

        tokio::select! {
            _ = publish_tick.tick() => {
                if let Err(error) = publish_own(&client, node, replica, keys, vault, day).await {
                    log::warn!("sync: publishing presence failed: {error}");
                }
            }
            result = changed.changed() => {
                if result.is_err() {
                    return;
                }
                if let Err(error) = publish_own(&client, node, replica, keys, vault, day).await {
                    log::warn!("sync: publishing presence failed: {error}");
                }
            }
            notification = notifications.next() => {
                let Some(notification) = notification else { break };
                if let nostr_sdk::client::ClientNotification::Event { event, .. } = notification {
                    if handle_incoming(node, replica, keys, vault, day, &event).await {
                        reconnect.notify_one();
                    }
                }
            }
        }
    }
}

/// Subscribes to today's and yesterday's mailbox, replacing any earlier
/// subscription (contracts/nostr-events.md: no `since`, renewed on day
/// rollover).
async fn resubscribe(
    client: &nostr_sdk::client::Client,
    replica: &crate::sync::replica::Replica,
    vault: [u8; 32],
    day: u32,
) -> Result<(), PresenceError> {
    let Some(roster) = read_roster(replica, vault)? else {
        return Ok(());
    };
    let (_, today) = mailbox_keys(&roster.content_key, day)?;
    let (_, yesterday) = mailbox_keys(&roster.content_key, day.saturating_sub(1))?;
    let _ = client.unsubscribe_all().await;
    let filter = nostr::filter::Filter::new()
        .kind(GIFT_WRAP_KIND)
        .pubkeys([today, yesterday]);
    client
        .subscribe(filter)
        .await
        .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    Ok(())
}

/// Publishes this device's own presence, unless its device list names only
/// itself (FR-007: a one-device vault only listens).
async fn publish_own(
    client: &nostr_sdk::client::Client,
    node: &crate::sync::endpoint::SyncNode,
    replica: &crate::sync::replica::Replica,
    keys: &DeviceKeys,
    vault: [u8; 32],
    day: u32,
) -> Result<(), PresenceError> {
    let Some(roster) = read_roster(replica, vault)? else {
        return Ok(());
    };
    if !roster.has_peers() {
        return Ok(());
    }
    let (_, mb_pk) = mailbox_keys(&roster.content_key, day)?;
    let addr = node.addr();
    let content = PresenceContent::own(
        keys.device_pubkey,
        keys.endpoint_id,
        addr.relay_urls().next().map(RelayUrl::to_string),
        addr.ip_addrs().copied().collect(),
        roster.list_generation,
    );
    let event = build(keys, &content, &mb_pk)?;
    client
        .send_event(&event)
        .await
        .map_err(|e| PresenceError::Crypto(e.to_string()))?;
    Ok(())
}

/// Opens and validates a received meeting, then records it (contracts/
/// nostr-events.md's receiver checks): fresh, signed by a device the
/// current effective list still names with that same endpoint, and (except
/// for a still-unlisted copy checking a newer list) not from a stranger.
/// Returns whether it recorded one, i.e. whether reconnect has something
/// new to dial.
async fn handle_incoming(
    node: &crate::sync::endpoint::SyncNode,
    replica: &crate::sync::replica::Replica,
    keys: &DeviceKeys,
    vault: [u8; 32],
    day: u32,
    event: &Event,
) -> bool {
    let roster = match read_roster(replica, vault) {
        Ok(Some(roster)) => roster,
        Ok(None) => return false,
        Err(error) => {
            log::warn!("sync: presence could not load the current state: {error}");
            return false;
        }
    };
    let Ok((mb_sk_today, _)) = mailbox_keys(&roster.content_key, day) else {
        return false;
    };
    let Ok((mb_sk_yesterday, _)) = mailbox_keys(&roster.content_key, day.saturating_sub(1)) else {
        return false;
    };
    let opened = open(event, &mb_sk_today).or_else(|_| open(event, &mb_sk_yesterday));
    let (sender, content) = match opened {
        Ok(pair) => pair,
        Err(_) => return false,
    };
    if content.device != sender || !content.is_fresh(now_ms()) {
        return false;
    }
    // The relay delivers this device's own meetings back to it, since it
    // subscribes to the same mailbox it publishes to. One from this key at
    // another endpoint is a copy of this installation (FR-030).
    if sender == keys.device_pubkey {
        if content.endpoint != keys.endpoint_id {
            node.flag(sender, crate::sync::problems::Problem::Duplicate)
                .await;
        }
        return false;
    }
    let Some(listed_endpoint) = roster
        .effective_devices
        .iter()
        .find_map(|(device, endpoint)| (*device == sender).then_some(*endpoint))
    else {
        // Meetings of unknown devices lead to no connection, except a
        // newer list from a copy of this vault — left for a later story
        // (FR-007's admission path) to act on; presence still records
        // nothing for them.
        return false;
    };
    // A listed key speaking from another endpoint, or sharing its device id
    // with another key, means two installations act as one device (R14,
    // FR-030): sync with it stops until that ends.
    if content.endpoint != listed_endpoint || roster.conflicting.contains(&sender) {
        node.flag(sender, crate::sync::problems::Problem::Duplicate)
            .await;
        return false;
    }

    let endpoint_addr = content.endpoint_addr().ok();
    if let Some(addr) = &endpoint_addr {
        node.note_presence(addr.clone()).await;
    }
    let write_result = replica.db().write(|tx| {
        record_seen(tx, &sender, now_ms(), endpoint_addr.as_ref())?;
        Ok(())
    });
    if let Err(error) = write_result {
        log::warn!("sync: recording presence for a device failed: {error}");
        return false;
    }
    // This write does not go through `VaultDb`, so it never reaches the
    // gate's sync-notify signal (spec 024, FR-010): the caller nudges
    // reconnect directly instead of leaving it to its own periodic tick.
    true
}

/// This device's current presence-relevant state: the content key it holds
/// (R9: skipping a generation wrapped for a removed device), the effective
/// list's generation, and the device keys that list currently names.
struct Roster {
    content_key: [u8; 32],
    list_generation: u64,
    /// Each device the list names, with the endpoint it names for it.
    effective_devices: Vec<([u8; 32], [u8; 32])>,
    /// Keys that share a device id with another key.
    conflicting: std::collections::HashSet<[u8; 32]>,
}

impl Roster {
    /// Whether the effective list names any device besides this one.
    fn has_peers(&self) -> bool {
        self.effective_devices.len() > 1
    }
}

/// Reads the [`Roster`] from storage; `None` while this device holds no
/// content key or no valid device list yet.
fn read_roster(
    replica: &crate::sync::replica::Replica,
    vault: [u8; 32],
) -> Result<Option<Roster>, PresenceError> {
    crate::storage::query::read(replica.db(), |r| {
        let vault_pubkey = crate::sync::keys::vault_pubkey(r)?.unwrap_or(vault);
        let rows = crate::sync::device_list::load_all(r)?;
        // Every list this device holds that still checks out, not just the
        // effective one: a same-generation fork that lost the tie-break
        // (FR-005/FR-043) must not make a key generation it saw look safe.
        let mut removed = Vec::new();
        for row in &rows {
            if let Ok(signed) = row.check(&vault_pubkey) {
                removed.extend(signed.list.removed.iter().map(|r| r.device_pubkey));
            }
        }
        let Some(key) = crate::sync::content_keys::current_key(r, &removed)? else {
            return Ok(None);
        };
        let valid = crate::sync::device_list::valid_lists(&rows, &vault_pubkey);
        let Some(effective) = crate::sync::device_list::effective(&valid) else {
            return Ok(None);
        };
        Ok(Some(Roster {
            content_key: *key.key,
            list_generation: effective.list.generation,
            effective_devices: effective
                .list
                .devices
                .iter()
                .map(|d| (d.device_pubkey, d.endpoint_id))
                .collect(),
            conflicting: crate::sync::device_list::uuid_conflicts(&valid),
        }))
    })
    .map_err(|e| PresenceError::Malformed(e.to_string()))
}

pub(crate) mod hex_bytes32 {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Writes the 32 bytes as lowercase hex.
    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&crate::sync::keys::hex(bytes))
    }

    /// Reads 32 bytes from a hex string.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(d)?;
        let bytes = super::decode_hex(&text).map_err(serde::de::Error::custom)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))
    }
}

mod hex_nonce {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Writes the 16-byte nonce as lowercase hex.
    pub fn serialize<S: Serializer>(bytes: &[u8; 16], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&crate::sync::keys::hex(bytes))
    }

    /// Reads a 16-byte nonce from a hex string.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 16], D::Error> {
        let text = String::deserialize(d)?;
        let bytes = super::decode_hex(&text).map_err(serde::de::Error::custom)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 16 bytes"))
    }
}

/// Decodes an even-length hex string (either case).
pub(crate) fn decode_hex(text: &str) -> Result<Vec<u8>, &'static str> {
    if !text.is_ascii() || !text.len().is_multiple_of(2) {
        return Err("not an even-length hex string");
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| "not hex"))
        .collect()
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
