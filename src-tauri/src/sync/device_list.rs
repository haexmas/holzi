//! The device list of a vault (spec 024, FR-005, FR-043, research R8).
//!
//! A device list is signed with the vault identity and stored insert-only in
//! the synchronized table `device_lists`. Rows are never changed or deleted,
//! so every list a device has ever seen stays checkable.
//!
//! Validity of one list:
//! - the vault identity's signature covers the whole payload, and the payload
//!   names this vault;
//! - `base_list_hash` points, except for a first list (generation 1), to a
//!   valid list of a lower generation, and the list carries forward every
//!   removal of that base list;
//! - no device is both listed and removed, no device key and no
//!   `vault_device_uuid` appears twice, and at least one device is a main
//!   device.
//!
//! `issued_by` is information only and is never checked: every main device
//! holds the same vault secret, so who issued a list cannot be proven.
//!
//! The effective list is the valid list of the highest generation; on a tie,
//! the one with the smallest hash. It alone decides which devices belong to
//! the vault and which are removed.

use std::collections::{BTreeMap, HashSet};

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::storage::query::Query;
use crate::sync::signing::{self, Domain, SigningError};

/// What a device may do in the vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Holds the vault secret; may add and remove devices.
    Main,
    /// Reads and writes all vault data but cannot change the device list.
    Linked,
}

/// One current device of the vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedDevice {
    pub device_pubkey: [u8; 32],
    pub endpoint_id: [u8; 32],
    pub role: Role,
    pub vault_device_uuid: Uuid,
    /// The device name, sealed with a content key
    /// (`crate::sync::content_keys::seal_name`).
    pub name_sealed: Vec<u8>,
    pub added_at: u64,
}

/// A device that was removed, with the limit its changes are accepted up to
/// (FR-028).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedDevice {
    pub device_pubkey: [u8; 32],
    pub vault_device_uuid: Uuid,
    pub limit_hlc: String,
    pub removed_at: u64,
}

/// The signed payload. Field order is part of the canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceList {
    pub vault: [u8; 32],
    pub generation: u64,
    pub devices: Vec<ListedDevice>,
    pub removed: Vec<RemovedDevice>,
    pub issued_by: [u8; 32],
    pub issued_at: u64,
    pub base_list_hash: Option<[u8; 32]>,
}

/// A device list with its encoding, hash and signature, as stored in
/// `device_lists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedList {
    pub hash: [u8; 32],
    pub payload: Vec<u8>,
    pub signature: [u8; 64],
    pub list: DeviceList,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ListError {
    #[error("the payload does not decode as a device list")]
    Malformed,
    #[error("the stored hash is not the hash of the payload")]
    HashMismatch,
    #[error("the list belongs to another vault")]
    ForeignVault,
    #[error("the vault identity's signature does not verify")]
    BadSignature,
    #[error("the stored generation differs from the payload's")]
    GenerationMismatch,
    #[error("only a list of generation 1 may have no base list, and it must have none")]
    BaseRule,
    #[error("the base list is unknown, invalid or not of a lower generation")]
    InvalidBase,
    #[error("the list drops a removal of its base list")]
    DropsRemoval,
    #[error("a device is both listed and removed")]
    ListedAndRemoved,
    #[error("a device key or vault device UUID appears twice")]
    Duplicate,
    #[error("the list has no main device")]
    NoMainDevice,
}

impl DeviceList {
    /// The canonical bytes the vault identity signs.
    pub fn encode(&self) -> Vec<u8> {
        signing::canonical(self).expect("a device list always encodes")
    }

    /// The structural rules that do not depend on other lists.
    pub fn check_structure(&self) -> Result<(), ListError> {
        if (self.base_list_hash.is_none()) != (self.generation == 1) {
            return Err(ListError::BaseRule);
        }
        let mut keys = HashSet::new();
        let mut uuids = HashSet::new();
        for device in &self.devices {
            if !keys.insert(device.device_pubkey) || !uuids.insert(device.vault_device_uuid) {
                return Err(ListError::Duplicate);
            }
        }
        let mut removed_keys = HashSet::new();
        let mut removed_uuids = HashSet::new();
        for removed in &self.removed {
            if keys.contains(&removed.device_pubkey) || uuids.contains(&removed.vault_device_uuid) {
                return Err(ListError::ListedAndRemoved);
            }
            if !removed_keys.insert(removed.device_pubkey)
                || !removed_uuids.insert(removed.vault_device_uuid)
            {
                return Err(ListError::Duplicate);
            }
        }
        if !self.devices.iter().any(|d| d.role == Role::Main) {
            return Err(ListError::NoMainDevice);
        }
        Ok(())
    }

    /// The listed entry of `device_pubkey`, if any.
    pub fn device(&self, device_pubkey: &[u8; 32]) -> Option<&ListedDevice> {
        self.devices
            .iter()
            .find(|d| &d.device_pubkey == device_pubkey)
    }

    /// Whether this list removes `device_pubkey`.
    pub fn removes(&self, device_pubkey: &[u8; 32]) -> bool {
        self.removed
            .iter()
            .any(|r| &r.device_pubkey == device_pubkey)
    }

    /// Whether `device_pubkey` is a main device in this list.
    pub fn is_main(&self, device_pubkey: &[u8; 32]) -> bool {
        self.device(device_pubkey)
            .is_some_and(|device| device.role == Role::Main)
    }
}

/// `SHA-256(payload)`, the list's primary key.
pub fn list_hash(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}

/// Encodes and signs `list` with the vault secret.
pub fn sign_list(list: DeviceList, vault_secret: &[u8; 32]) -> Result<SignedList, SigningError> {
    let payload = list.encode();
    let signature = signing::sign(Domain::DeviceList, &payload, vault_secret)?;
    Ok(SignedList {
        hash: list_hash(&payload),
        payload,
        signature,
        list,
    })
}

/// A `device_lists` row as read, before any check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredList {
    pub hash: Vec<u8>,
    pub generation: i64,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

impl StoredList {
    /// Checks the row on its own: hash, signature, vault, generation and
    /// structure. The rules about the base list need the other lists; see
    /// [`valid_lists`].
    pub fn check(&self, vault_pubkey: &[u8; 32]) -> Result<SignedList, ListError> {
        let hash: [u8; 32] = self
            .hash
            .as_slice()
            .try_into()
            .map_err(|_| ListError::HashMismatch)?;
        if hash != list_hash(&self.payload) {
            return Err(ListError::HashMismatch);
        }
        let signature: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| ListError::BadSignature)?;
        signing::verify(Domain::DeviceList, &self.payload, &signature, vault_pubkey)
            .map_err(|_| ListError::BadSignature)?;
        let list: DeviceList =
            postcard::from_bytes(&self.payload).map_err(|_| ListError::Malformed)?;
        if &list.vault != vault_pubkey {
            return Err(ListError::ForeignVault);
        }
        if i64::try_from(list.generation).ok() != Some(self.generation) {
            return Err(ListError::GenerationMismatch);
        }
        list.check_structure()?;
        Ok(SignedList {
            hash,
            payload: self.payload.clone(),
            signature,
            list,
        })
    }
}

/// Every valid list among `rows`, keyed by hash. A list whose base list is
/// missing or invalid is itself invalid.
pub fn valid_lists(rows: &[StoredList], vault_pubkey: &[u8; 32]) -> BTreeMap<[u8; 32], SignedList> {
    let mut checked: Vec<SignedList> = rows
        .iter()
        .filter_map(|row| row.check(vault_pubkey).ok())
        .collect();
    // A base list always has a lower generation, so it is decided first.
    checked.sort_by_key(|signed| signed.list.generation);
    let mut valid: BTreeMap<[u8; 32], SignedList> = BTreeMap::new();
    for signed in checked {
        if base_rules(&signed.list, &valid).is_ok() {
            valid.insert(signed.hash, signed);
        }
    }
    valid
}

/// The rules between a list and its base list.
fn base_rules(list: &DeviceList, valid: &BTreeMap<[u8; 32], SignedList>) -> Result<(), ListError> {
    let Some(base_hash) = list.base_list_hash else {
        return Ok(());
    };
    let base = valid.get(&base_hash).ok_or(ListError::InvalidBase)?;
    if base.list.generation >= list.generation {
        return Err(ListError::InvalidBase);
    }
    let carried: HashSet<[u8; 32]> = list.removed.iter().map(|r| r.device_pubkey).collect();
    if base
        .list
        .removed
        .iter()
        .any(|r| !carried.contains(&r.device_pubkey))
    {
        return Err(ListError::DropsRemoval);
    }
    Ok(())
}

/// The effective list: highest generation, on a tie the smallest hash.
pub fn effective(valid: &BTreeMap<[u8; 32], SignedList>) -> Option<&SignedList> {
    // Iterating the map in hash order and keeping the first of the highest
    // generation yields the smallest hash on a tie.
    valid
        .values()
        .fold(None, |best: Option<&SignedList>, signed| match best {
            Some(best) if best.list.generation >= signed.list.generation => Some(best),
            _ => Some(signed),
        })
}

/// Device keys that share a `vault_device_uuid` with another key across the
/// valid lists of the highest generation (FR-030). One list never has that
/// (`check_structure`), so it only shows after a fork, when two main devices
/// added different keys for one device id. Neither may sync until a merged
/// list settles it.
pub fn uuid_conflicts(valid: &BTreeMap<[u8; 32], SignedList>) -> HashSet<[u8; 32]> {
    let top = valid.values().map(|s| s.list.generation).max();
    let mut owners: BTreeMap<Uuid, HashSet<[u8; 32]>> = BTreeMap::new();
    for signed in valid.values().filter(|s| Some(s.list.generation) == top) {
        for device in &signed.list.devices {
            owners
                .entry(device.vault_device_uuid)
                .or_default()
                .insert(device.device_pubkey);
        }
    }
    owners
        .into_values()
        .filter(|keys| keys.len() > 1)
        .flatten()
        .collect()
}

/// The next generation over `effective`, merging in the devices of
/// `others` (lists of the same or a lower generation that lost) that the
/// effective list neither lists nor removes (FR-043). The effective list's
/// removals are carried forward. The caller signs the result.
pub fn merge_next(
    effective: &SignedList,
    others: &[&SignedList],
    issued_by: [u8; 32],
    issued_at: u64,
) -> DeviceList {
    let base = &effective.list;
    let mut devices = base.devices.clone();
    for other in others {
        for device in &other.list.devices {
            let known = devices.iter().any(|d| {
                d.device_pubkey == device.device_pubkey
                    || d.vault_device_uuid == device.vault_device_uuid
            });
            let removed = base.removed.iter().any(|r| {
                r.device_pubkey == device.device_pubkey
                    || r.vault_device_uuid == device.vault_device_uuid
            });
            if !known && !removed {
                devices.push(device.clone());
            }
        }
    }
    DeviceList {
        vault: base.vault,
        generation: base.generation + 1,
        devices,
        removed: base.removed.clone(),
        issued_by,
        issued_at,
        base_list_hash: Some(effective.hash),
    }
}

/// Whether list `a` ranks before list `b`: a higher generation, or the same
/// generation with a smaller hash.
pub fn ranks_before(a: (u64, &[u8; 32]), b: (u64, &[u8; 32])) -> bool {
    a.0 > b.0 || (a.0 == b.0 && a.1 < b.1)
}

/// The list `hash` and its base lists, oldest first: what a device needs to
/// check it.
pub fn ancestry<'a>(
    valid: &'a BTreeMap<[u8; 32], SignedList>,
    hash: &[u8; 32],
) -> Vec<&'a SignedList> {
    let mut chain = Vec::new();
    let mut next = valid.get(hash);
    while let Some(signed) = next {
        chain.push(signed);
        next = signed.list.base_list_hash.and_then(|base| valid.get(&base));
    }
    chain.reverse();
    chain
}

/// Checks a list another device sent: the same checks as a stored row.
pub fn check_pushed(
    payload: &[u8],
    signature: &[u8; 64],
    vault_pubkey: &[u8; 32],
) -> Result<SignedList, ListError> {
    let list: DeviceList = postcard::from_bytes(payload).map_err(|_| ListError::Malformed)?;
    StoredList {
        hash: list_hash(payload).to_vec(),
        generation: i64::try_from(list.generation).map_err(|_| ListError::GenerationMismatch)?,
        payload: payload.to_vec(),
        signature: signature.to_vec(),
    }
    .check(vault_pubkey)
}

/// Stores a signed list. Idempotent: a list that is already stored stays as it is.
pub fn insert(tx: &mut CrdtTransaction<'_>, signed: &SignedList) -> haex_crdt::Result<()> {
    let generation = i64::try_from(signed.list.generation)
        .map_err(|_| haex_crdt::Error::consumer("device list generation beyond i64"))?;
    tx.execute(
        "INSERT OR IGNORE INTO device_lists (list_hash, generation, payload, signature) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            signed.hash.as_slice(),
            generation,
            signed.payload,
            signed.signature.as_slice()
        ],
    )?;
    Ok(())
}

/// Every stored list, unchecked.
pub fn load_all(q: &mut impl Query) -> haex_crdt::Result<Vec<StoredList>> {
    q.query_map(
        "SELECT list_hash, generation, payload, signature FROM device_lists",
        &[],
        |r| {
            Ok(StoredList {
                hash: r.get(0)?,
                generation: r.get(1)?,
                payload: r.get(2)?,
                signature: r.get(3)?,
            })
        },
    )
}

#[cfg(test)]
#[path = "device_list_tests.rs"]
mod tests;
