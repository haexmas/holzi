//! `pending_links_no_sync`: a link that has begun and not finished, kept so
//! that a restart can finish it without the used-up code (data-model.md,
//! contracts/sync-protocol.md). Device-local, never synchronized.

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;

use crate::storage::query::Query;

/// A pending record older than this without completing is dropped (R20).
pub const MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;

/// Where a link stands on this side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Transferring,
    AwaitingPublication,
    Completed,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Transferring => "transferring",
            State::AwaitingPublication => "awaiting_publication",
            State::Completed => "completed",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "transferring" => Some(State::Transferring),
            "awaiting_publication" => Some(State::AwaitingPublication),
            "completed" => Some(State::Completed),
            _ => None,
        }
    }
}

/// A row of `pending_links_no_sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub link_id: [u8; 32],
    pub peer_device: [u8; 32],
    pub list_hash: [u8; 32],
    pub list_payload: Vec<u8>,
    pub list_signature: [u8; 64],
    /// `main` when the new device becomes a main device.
    pub as_main: bool,
    pub resume_secret: [u8; 32],
    pub state: State,
    pub created_at: i64,
}

/// Stores `pending`, replacing a record with the same link id.
pub fn store(tx: &mut CrdtTransaction<'_>, pending: &Pending) -> haex_crdt::Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO pending_links_no_sync \
           (link_id, peer_device_pubkey, new_list_hash, new_list_payload, new_list_signature, \
            role, resume_secret, state, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            pending.link_id.as_slice(),
            pending.peer_device.as_slice(),
            pending.list_hash.as_slice(),
            pending.list_payload,
            pending.list_signature.as_slice(),
            if pending.as_main { "main" } else { "linked" },
            pending.resume_secret.as_slice(),
            pending.state.as_str(),
            pending.created_at,
        ],
    )?;
    Ok(())
}

/// Moves a record to `state`.
pub fn set_state(
    tx: &mut CrdtTransaction<'_>,
    link_id: &[u8; 32],
    state: State,
) -> haex_crdt::Result<()> {
    tx.execute(
        "UPDATE pending_links_no_sync SET state = ?2 WHERE link_id = ?1",
        params![link_id.as_slice(), state.as_str()],
    )?;
    Ok(())
}

/// Deletes a record; the link finished or was cancelled.
pub fn delete(tx: &mut CrdtTransaction<'_>, link_id: &[u8; 32]) -> haex_crdt::Result<()> {
    tx.execute(
        "DELETE FROM pending_links_no_sync WHERE link_id = ?1",
        params![link_id.as_slice()],
    )?;
    Ok(())
}

/// Deletes every record created before `now_ms - MAX_AGE_MS`.
pub fn delete_stale(tx: &mut CrdtTransaction<'_>, now_ms: i64) -> haex_crdt::Result<usize> {
    tx.execute(
        "DELETE FROM pending_links_no_sync WHERE created_at < ?1",
        params![now_ms.saturating_sub(MAX_AGE_MS)],
    )
}

/// Every stored record.
pub fn load_all(q: &mut impl Query) -> haex_crdt::Result<Vec<Pending>> {
    type Row = (
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        String,
        Vec<u8>,
        String,
        i64,
    );
    let rows: Vec<Row> = q.query_map(
        "SELECT link_id, peer_device_pubkey, new_list_hash, new_list_payload, \
                new_list_signature, role, resume_secret, state, created_at \
         FROM pending_links_no_sync ORDER BY created_at",
        &[],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        },
    )?;
    rows.into_iter()
        .map(
            |(link_id, peer, hash, payload, signature, role, secret, state, created_at)| {
                let bad = |what: &str| haex_crdt::Error::consumer(format!("pending link: {what}"));
                Ok(Pending {
                    link_id: link_id.try_into().map_err(|_| bad("link_id"))?,
                    peer_device: peer.try_into().map_err(|_| bad("peer"))?,
                    list_hash: hash.try_into().map_err(|_| bad("list hash"))?,
                    list_payload: payload,
                    list_signature: signature.try_into().map_err(|_| bad("signature"))?,
                    as_main: role == "main",
                    resume_secret: secret.try_into().map_err(|_| bad("secret"))?,
                    state: State::parse(&state).ok_or_else(|| bad("state"))?,
                    created_at,
                })
            },
        )
        .collect()
}
