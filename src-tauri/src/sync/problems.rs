//! Why sync with a device is halted (spec 024, FR-029, FR-030, research
//! R14), kept in the device-local column `device_presence_no_sync.problem`.
//!
//! A problem is set from evidence this device gathered itself (a verified
//! handshake or a fresh, authenticated presence meeting) and cleared by the
//! next successful handshake with that device. Presence never clears it: a
//! device that is reachable can still be one this device must not sync with.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use haex_crdt::rusqlite::params;

use crate::storage::query::{self, Query};
use crate::sync::replica::Replica;

/// What halts sync with a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The device runs a sync or vault schema this build cannot exchange
    /// changes with (FR-029).
    IncompatibleVersion,
    /// Two installations present themselves as the same device (FR-030).
    Duplicate,
}

impl Problem {
    /// The value in the `problem` column and in the `DeviceProblem` type of
    /// contracts/tauri-commands.md.
    pub fn as_str(self) -> &'static str {
        match self {
            Problem::IncompatibleVersion => "incompatible_version",
            Problem::Duplicate => "duplicate",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "incompatible_version" => Some(Problem::IncompatibleVersion),
            "duplicate" => Some(Problem::Duplicate),
            _ => None,
        }
    }
}

/// How long a device stays halted after the last sign of a duplicate. The
/// evidence is a second endpoint answering to the same device key; once it
/// stops appearing, the device syncs again without the user doing anything.
const DUPLICATE_HOLD: Duration = Duration::from_secs(10 * 60);

/// Devices seen as duplicates recently, in memory only: a restart forgets
/// them, and the next sign of a duplicate brings them back.
#[derive(Debug, Default)]
pub struct DuplicateWatch {
    seen: Mutex<HashMap<[u8; 32], Instant>>,
}

impl DuplicateWatch {
    /// Notes fresh evidence of a duplicate of `device`.
    pub fn note(&self, device: [u8; 32]) {
        self.lock().insert(device, Instant::now());
    }

    /// Whether `device` is held back right now.
    pub fn holds(&self, device: &[u8; 32]) -> bool {
        let mut seen = self.lock();
        match seen.get(device) {
            Some(at) if at.elapsed() < DUPLICATE_HOLD => true,
            Some(_) => {
                seen.remove(device);
                false
            }
            None => false,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<[u8; 32], Instant>> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Sets `problem` for `device`; returns whether that changed anything.
pub fn set(replica: &Replica, device: &[u8; 32], problem: Problem) -> haex_crdt::Result<bool> {
    replica.db().write(|tx| {
        let known: Option<i64> = tx.query_row(
            "SELECT 1 FROM device_presence_no_sync WHERE device_pubkey = ?1",
            params![device.as_slice()],
            |r| r.get(0),
        )?;
        if known.is_some() {
            let changed = tx.execute(
                "UPDATE device_presence_no_sync SET problem = ?2 \
                 WHERE device_pubkey = ?1 AND (problem IS NULL OR problem != ?2)",
                params![device.as_slice(), problem.as_str()],
            )?;
            return Ok(changed > 0);
        }
        // A device never seen keeps `last_seen = 0`; only a meeting or a
        // handshake gives it a real time.
        tx.execute(
            "INSERT INTO device_presence_no_sync (device_pubkey, last_seen, endpoint_addr, problem) \
             VALUES (?1, 0, NULL, ?2)",
            params![device.as_slice(), problem.as_str()],
        )?;
        Ok(true)
    })
}

/// Clears any problem of `device`; returns whether there was one.
pub fn clear(replica: &Replica, device: &[u8; 32]) -> haex_crdt::Result<bool> {
    replica.db().write(|tx| {
        let changed = tx.execute(
            "UPDATE device_presence_no_sync SET problem = NULL \
             WHERE device_pubkey = ?1 AND problem IS NOT NULL",
            params![device.as_slice()],
        )?;
        Ok(changed > 0)
    })
}

/// The problem recorded for `device`, if any.
pub fn of(q: &mut impl Query, device: &[u8; 32]) -> haex_crdt::Result<Option<Problem>> {
    let text: Option<Option<String>> = q.query_row(
        "SELECT problem FROM device_presence_no_sync WHERE device_pubkey = ?1",
        params![device.as_slice()],
        |r| r.get(0),
    )?;
    Ok(text.flatten().and_then(|t| Problem::parse(&t)))
}

/// The device the effective list names with `endpoint`, if any. The list is
/// signed by the vault identity and the endpoint id is what the transport
/// authenticated, so this names a device without trusting the peer's word.
pub fn device_at_endpoint(
    replica: &Replica,
    vault: [u8; 32],
    endpoint: &[u8; 32],
) -> haex_crdt::Result<Option<[u8; 32]>> {
    query::read(replica.db(), |r| {
        let valid =
            crate::sync::device_list::valid_lists(&crate::sync::device_list::load_all(r)?, &vault);
        Ok(
            crate::sync::device_list::effective(&valid).and_then(|signed| {
                signed
                    .list
                    .devices
                    .iter()
                    .find(|d| &d.endpoint_id == endpoint)
                    .map(|d| d.device_pubkey)
            }),
        )
    })
}

#[cfg(test)]
#[path = "problems_tests.rs"]
mod tests;
