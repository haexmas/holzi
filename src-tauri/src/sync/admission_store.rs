//! The table of open requests to join (spec 024, user story 7, FR-045, R20):
//! keeping a request, the sweep every device runs after a merge, and reading
//! what waits for a decision. Part of [`super`].

use std::collections::HashSet;

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use uuid::Uuid;

use super::{settled, Request, MAX_AGE_MS, OPEN, OPEN_LIMIT, REJECTED};
use crate::storage::query::Query;
use crate::sync::device_list;
use crate::sync::keys;

/// Keeps `request` as an open request. A newer request of a copy that is
/// still open replaces its older one; a refused one stays refused (so the
/// copy's repeats do not bring it back). `true` when the table changed.
pub fn store(tx: &mut CrdtTransaction<'_>, request: &Request) -> haex_crdt::Result<bool> {
    let existing: Option<(String, i64)> = tx.query_row(
        "SELECT state, requested_at FROM admission_requests WHERE device_pubkey = ?1",
        params![request.device.as_slice()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let requested_at = to_i64(request.requested_at);
    match existing {
        None => {
            tx.execute(
                "INSERT INTO admission_requests \
                   (device_pubkey, vault_device_uuid, endpoint_id, name, requested_at, \
                    signature, state) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    request.device.as_slice(),
                    request.vault_device_uuid.to_string(),
                    request.endpoint.as_slice(),
                    request.name,
                    requested_at,
                    request.signature.as_slice(),
                    OPEN,
                ],
            )?;
            Ok(true)
        }
        Some((state, at)) if state == OPEN && requested_at > at => {
            tx.execute(
                "UPDATE admission_requests \
                 SET endpoint_id = ?2, name = ?3, requested_at = ?4, signature = ?5 \
                 WHERE device_pubkey = ?1",
                params![
                    request.device.as_slice(),
                    request.endpoint.as_slice(),
                    request.name,
                    requested_at,
                    request.signature.as_slice(),
                ],
            )?;
            Ok(true)
        }
        Some(_) => Ok(false),
    }
}

/// What every device does after a merge: drops settled, stale and overflowing
/// requests the same way, so the same merged set ends in the same state on
/// every device (R20). Only the 20 smallest open requests by `(requested_at,
/// device_pubkey)` stay open; the others are refused. `true` when it changed
/// the table.
pub fn sweep(
    tx: &mut CrdtTransaction<'_>,
    now_ms: u64,
    settled: &HashSet<[u8; 32]>,
) -> haex_crdt::Result<bool> {
    let rows: Vec<(Vec<u8>, i64, String)> = tx.query_map(
        "SELECT device_pubkey, requested_at, state FROM admission_requests",
        &[],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let oldest = to_i64(now_ms.saturating_sub(MAX_AGE_MS));
    let mut changed = false;
    let mut open = Vec::new();
    for (device, requested_at, state) in rows {
        let is_settled = <[u8; 32]>::try_from(device.as_slice())
            .map(|d| settled.contains(&d))
            .unwrap_or(true);
        if is_settled || requested_at < oldest {
            tx.execute(
                "DELETE FROM admission_requests WHERE device_pubkey = ?1",
                params![device],
            )?;
            changed = true;
        } else if state == OPEN {
            open.push((requested_at, device));
        }
    }
    open.sort();
    for (_, device) in open.into_iter().skip(OPEN_LIMIT) {
        tx.execute(
            "UPDATE admission_requests SET state = ?2 WHERE device_pubkey = ?1 AND state = ?3",
            params![device, REJECTED, OPEN],
        )?;
        changed = true;
    }
    Ok(changed)
}

/// [`sweep`] on the effective list this device holds now; nothing without
/// one. Run when a vault opens and after a merge brought requests or lists.
pub fn sweep_now(replica: &crate::sync::replica::Replica, now_ms: u64) -> haex_crdt::Result<bool> {
    replica.db().write(|tx| {
        let Some(vault) = keys::vault_pubkey(tx)? else {
            return Ok(false);
        };
        let valid = device_list::valid_lists(&device_list::load_all(tx)?, &vault);
        let Some(effective) = device_list::effective(&valid) else {
            return Ok(false);
        };
        sweep(tx, now_ms, &settled(effective))
    })
}

/// One open request, as the main device lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub device: [u8; 32],
    pub name: String,
    pub requested_at: u64,
}

/// The open requests from devices the list has not settled, oldest first.
pub fn load_open(
    q: &mut impl Query,
    settled: &HashSet<[u8; 32]>,
) -> haex_crdt::Result<Vec<OpenRequest>> {
    let rows: Vec<(Vec<u8>, String, i64)> = q.query_map(
        "SELECT device_pubkey, name, requested_at FROM admission_requests \
         WHERE state = ?1 ORDER BY requested_at, device_pubkey",
        params![OPEN],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|(device, name, requested_at)| {
            let device = <[u8; 32]>::try_from(device.as_slice()).ok()?;
            (!settled.contains(&device)).then(|| OpenRequest {
                device,
                name,
                requested_at: u64::try_from(requested_at).unwrap_or(0),
            })
        })
        .collect())
}

pub(super) fn load_request(
    q: &mut impl Query,
    device: &[u8; 32],
) -> haex_crdt::Result<Option<Request>> {
    type Row = (Vec<u8>, String, Vec<u8>, String, i64, Vec<u8>);
    let row: Option<Row> = q.query_row(
        "SELECT device_pubkey, vault_device_uuid, endpoint_id, name, requested_at, signature \
         FROM admission_requests WHERE device_pubkey = ?1 AND state = ?2",
        params![device.as_slice(), OPEN],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )?;
    let Some((device, uuid, endpoint, name, requested_at, signature)) = row else {
        return Ok(None);
    };
    let malformed = || haex_crdt::Error::consumer("admission_requests holds a malformed row");
    Ok(Some(Request {
        device: device.as_slice().try_into().map_err(|_| malformed())?,
        endpoint: endpoint.as_slice().try_into().map_err(|_| malformed())?,
        vault_device_uuid: Uuid::parse_str(&uuid).map_err(|_| malformed())?,
        name,
        requested_at: u64::try_from(requested_at).unwrap_or(0),
        signature: signature.as_slice().try_into().map_err(|_| malformed())?,
    }))
}

pub(super) fn delete_request(
    tx: &mut CrdtTransaction<'_>,
    device: &[u8; 32],
) -> haex_crdt::Result<()> {
    tx.execute(
        "DELETE FROM admission_requests WHERE device_pubkey = ?1",
        params![device.as_slice()],
    )?;
    Ok(())
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
