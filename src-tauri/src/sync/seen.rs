//! When each device was last online, as this device knows it (spec 024,
//! FR-033, data-model.md `device_presence_no_sync.last_seen`).
//!
//! The newest of three sources: this device's own session with it (a
//! session starting or ending), a presence meeting, and a third device,
//! which reports what it knows in its `Progress` messages. Times only move
//! forward, and a report from the future is clamped to now, so a device
//! with a wrong clock cannot push another device's "last online" ahead.

use haex_crdt::{rusqlite::params, CrdtTransaction};

use crate::storage::query::Query;
use crate::sync::replica::Replica;

/// How far into the future a reported time may lie before it counts as now.
const MAX_FUTURE_MS: u64 = 30_000;

/// Raises `device`'s last-seen time to `at_ms`; returns whether it moved.
pub fn touch(replica: &Replica, device: &[u8; 32], at_ms: u64) -> haex_crdt::Result<bool> {
    replica.db().write(|tx| touch_in(tx, device, at_ms))
}

fn touch_in(
    tx: &mut CrdtTransaction<'_>,
    device: &[u8; 32],
    at_ms: u64,
) -> haex_crdt::Result<bool> {
    let at = i64::try_from(at_ms).unwrap_or(i64::MAX);
    let known: Option<i64> = tx.query_row(
        "SELECT last_seen FROM device_presence_no_sync WHERE device_pubkey = ?1",
        params![device.as_slice()],
        |r| r.get(0),
    )?;
    match known {
        Some(seen) if seen >= at => Ok(false),
        Some(_) => {
            tx.execute(
                "UPDATE device_presence_no_sync SET last_seen = ?2 WHERE device_pubkey = ?1",
                params![device.as_slice(), at],
            )?;
            Ok(true)
        }
        None => {
            tx.execute(
                "INSERT INTO device_presence_no_sync \
                   (device_pubkey, last_seen, endpoint_addr, problem) \
                 VALUES (?1, ?2, NULL, NULL)",
                params![device.as_slice(), at],
            )?;
            Ok(true)
        }
    }
}

/// Folds what a third device reported into the stored times. `own` is never
/// taken from a report: this device knows when it was online. Returns
/// whether anything moved.
pub fn merge(
    replica: &Replica,
    reports: &[([u8; 32], u64)],
    own: &[u8; 32],
    now_ms: u64,
) -> haex_crdt::Result<bool> {
    let ceiling = now_ms.saturating_add(MAX_FUTURE_MS);
    replica.db().write(|tx| {
        let mut moved = false;
        for (device, at) in reports {
            if device == own {
                continue;
            }
            let at = if *at > ceiling { now_ms } else { *at };
            // A device a report names but this device never met gets a row too.
            moved |= touch_in(tx, device, at)?;
        }
        Ok(moved)
    })
}

/// What this device tells a peer it knows: every stored time, and itself as
/// of now.
pub fn snapshot(
    q: &mut impl Query,
    own: &[u8; 32],
    now_ms: u64,
) -> haex_crdt::Result<Vec<([u8; 32], u64)>> {
    let rows: Vec<(Vec<u8>, i64)> = q.query_map(
        "SELECT device_pubkey, last_seen FROM device_presence_no_sync WHERE last_seen > 0",
        &[],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut out: Vec<([u8; 32], u64)> = rows
        .into_iter()
        .filter_map(|(device, at)| Some((device.try_into().ok()?, u64::try_from(at).ok()?)))
        .filter(|(device, _)| device != own)
        .collect();
    out.push((*own, now_ms));
    Ok(out)
}

#[cfg(test)]
#[path = "seen_tests.rs"]
mod tests;
