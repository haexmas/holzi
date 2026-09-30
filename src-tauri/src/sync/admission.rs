//! Copies of the vault file and their admission (spec 024, user story 7,
//! FR-044, FR-045, research R12, R20).
//!
//! A copy opens with keys of its own (genesis creates them; the source's row
//! stays untouched). A copy of a main device holds the vault secret and
//! enrolls itself as a main device ([`enroll_as_main`]). A copy of a linked
//! device cannot sign a device list: it asks the devices of the vault to
//! admit it with a request signed by its new device key ([`Request`]), sent
//! with its presence. Every listed device that receives a valid request keeps
//! it in the synchronized `admission_requests` ([`store`]), so it reaches
//! every main device. A main device admits it ([`decide`]) into a new device
//! list with the content keys wrapped for it, or refuses it; nothing admits a
//! copy without that decision (FR-045).
//!
//! The request carries the copy's `vault_device_uuid` (its CRDT node id)
//! beyond what contracts/nostr-events.md first listed, because the device
//! list names a device by it and the changes the copy made while it waited
//! carry it. Its transient reachability (`iroh_relay`, `addrs`) is not part
//! of the signature and is never stored.

use std::collections::HashSet;

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use iroh::{EndpointAddr, RelayUrl};
use nostr::event::Kind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storage::query::Query;
use crate::sync::content_keys;
use crate::sync::device_list::{self, DeviceList, ListedDevice, Role, SignedList};
use crate::sync::envelopes;
use crate::sync::keys::{self, DeviceKeys};
use crate::sync::presence::hex_bytes32;
use crate::sync::signing::{self, lp, Domain};

/// The device-scoped preference that tells the user, once, that this copy
/// enrolled itself as a main device (FR-044).
pub const PREF_ENROLLED_AS_MAIN: &str = "sync.copy.enrolled_as_main";
/// The inner event kind of a request (contracts/nostr-events.md).
pub(crate) const ADMISSION_KIND: Kind = Kind::Custom(24101);
/// Open requests kept after a merge; the rest are refused (R20).
pub const OPEN_LIMIT: usize = 20;
/// An unanswered or refused request is dropped after this long (R20).
pub const MAX_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;
/// How far ahead of this device's clock a request may claim to be.
const MAX_FUTURE_MS: u64 = 30_000;
/// The longest name a request may carry.
const MAX_NAME_BYTES: usize = 256;

const OPEN: &str = "open";
const REJECTED: &str = "rejected";

#[derive(Debug, thiserror::Error)]
pub enum AdmissionError {
    /// Only a main device, which holds the vault secret, admits or refuses.
    #[error("this device is not a main device")]
    NotMainDevice,
    #[error("there is no open request from this device")]
    NoRequest,
    #[error("the vault has no valid device list")]
    NoDeviceList,
    #[error("this device holds no content key to wrap for the new device")]
    NoContentKey,
    #[error(transparent)]
    Crdt(#[from] haex_crdt::Error),
    #[error("a database task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

impl From<AdmissionError> for crate::error::HolziError {
    fn from(error: AdmissionError) -> Self {
        use crate::error::HolziError;
        match error {
            AdmissionError::NotMainDevice => HolziError::NotMainDevice,
            AdmissionError::Crdt(error) => error.into(),
            AdmissionError::Join(error) => HolziError::CrdtInit {
                reason: format!("admission task: {error}"),
            },
            other => HolziError::InvalidInput {
                reason: other.to_string(),
            },
        }
    }
}

/// A copy's request to be admitted, with the signature of its device key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub device: [u8; 32],
    pub endpoint: [u8; 32],
    pub vault_device_uuid: Uuid,
    pub name: String,
    pub requested_at: u64,
    pub signature: [u8; 64],
}

impl Request {
    /// A request signed by `keys`.
    pub fn sign(
        keys: &DeviceKeys,
        vault_device_uuid: Uuid,
        name: String,
        requested_at: u64,
    ) -> Result<Self, signing::SigningError> {
        let bytes = signed_bytes(
            &keys.device_pubkey,
            &keys.endpoint_id,
            &vault_device_uuid,
            &name,
            requested_at,
        );
        let signature = signing::sign(Domain::Admission, &bytes, &keys.device_secret)?;
        Ok(Self {
            device: keys.device_pubkey,
            endpoint: keys.endpoint_id,
            vault_device_uuid,
            name,
            requested_at,
            signature,
        })
    }

    /// Whether the signature is the device's own over exactly these fields.
    pub fn verify(&self) -> Result<(), signing::SigningError> {
        let bytes = signed_bytes(
            &self.device,
            &self.endpoint,
            &self.vault_device_uuid,
            &self.name,
            self.requested_at,
        );
        signing::verify(Domain::Admission, &bytes, &self.signature, &self.device)
    }
}

/// `lp(device) ‖ lp(endpoint) ‖ lp(vault_device_uuid) ‖ lp(name) ‖ requested_at`.
fn signed_bytes(
    device: &[u8; 32],
    endpoint: &[u8; 32],
    vault_device_uuid: &Uuid,
    name: &str,
    requested_at: u64,
) -> Vec<u8> {
    [
        lp(device),
        lp(endpoint),
        lp(vault_device_uuid.as_bytes()),
        lp(name.as_bytes()),
        requested_at.to_be_bytes().to_vec(),
    ]
    .concat()
}

/// The JSON of the inner event (contracts/nostr-events.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    pub v: u8,
    #[serde(with = "hex_bytes32")]
    pub device: [u8; 32],
    #[serde(with = "hex_bytes32")]
    pub endpoint: [u8; 32],
    pub vault_device_uuid: Uuid,
    pub name: String,
    pub requested_at: u64,
    #[serde(with = "hex_bytes64")]
    pub sig: [u8; 64],
    /// Where the copy can be reached right now: not signed, not stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iroh_relay: Option<String>,
    #[serde(default)]
    pub addrs: Vec<String>,
}

impl Content {
    /// The event content for `request`, reachable at `addr`.
    pub fn new(request: &Request, addr: &EndpointAddr) -> Self {
        Self {
            v: 1,
            device: request.device,
            endpoint: request.endpoint,
            vault_device_uuid: request.vault_device_uuid,
            name: request.name.clone(),
            requested_at: request.requested_at,
            sig: request.signature,
            iroh_relay: addr.relay_urls().next().map(RelayUrl::to_string),
            addrs: addr.ip_addrs().map(|a| a.to_string()).collect(),
        }
    }

    /// The request this content carries, once `sender` (the seal's signer) is
    /// the device it names and its fields are sane, fresh and signed.
    pub fn request(&self, sender: &[u8; 32], now_ms: u64) -> Option<Request> {
        if self.v != 1
            || &self.device != sender
            || self.name.len() > MAX_NAME_BYTES
            || self.requested_at > now_ms.saturating_add(MAX_FUTURE_MS)
            || now_ms.saturating_sub(self.requested_at) > MAX_AGE_MS
        {
            return None;
        }
        let request = Request {
            device: self.device,
            endpoint: self.endpoint,
            vault_device_uuid: self.vault_device_uuid,
            name: self.name.clone(),
            requested_at: self.requested_at,
            signature: self.sig,
        };
        request.verify().ok()?;
        Some(request)
    }

    /// The address the copy announced, for dialing it once it is admitted.
    pub fn endpoint_addr(&self) -> Option<EndpointAddr> {
        crate::sync::presence::endpoint_addr_of(
            &self.endpoint,
            self.iroh_relay.as_deref(),
            &self.addrs,
        )
        .ok()
    }
}

mod hex_bytes64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&crate::sync::keys::hex(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let text = String::deserialize(d)?;
        let bytes = crate::sync::presence::decode_hex(&text).map_err(serde::de::Error::custom)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 64 bytes"))
    }
}

/// Devices the effective list names or has removed: requests from them are
/// settled and never shown.
pub fn settled(list: &SignedList) -> HashSet<[u8; 32]> {
    list.list
        .devices
        .iter()
        .map(|d| d.device_pubkey)
        .chain(list.list.removed.iter().map(|r| r.device_pubkey))
        .collect()
}

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

/// What a main device decided about an open request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided {
    /// The copy is on the new list, generation `list_generation`.
    Admitted {
        list_generation: u64,
        endpoint_id: [u8; 32],
    },
    Refused,
    /// The copy was on the list already: only the stale request went.
    AlreadyListed,
    /// The request named a removed device or did not verify: it went, nobody
    /// was admitted.
    Dropped,
}

/// Admits (`admit`) or refuses the open request of `target`, in one
/// transaction. Admitting puts the copy on a device list of the next
/// generation as a linked device and wraps every content key generation this
/// device holds for it (FR-045).
pub fn decide(
    replica: &crate::sync::replica::Replica,
    own: &DeviceKeys,
    target: &[u8; 32],
    admit: bool,
    now_ms: u64,
) -> Result<Decided, AdmissionError> {
    let (own, target) = (own.clone(), *target);
    replica
        .db()
        .write(|tx| {
            let fail = |error: AdmissionError| haex_crdt::Error::consumer(error);
            let vault_secret =
                keys::vault_secret(tx)?.ok_or_else(|| fail(AdmissionError::NotMainDevice))?;
            let request =
                load_request(tx, &target)?.ok_or_else(|| fail(AdmissionError::NoRequest))?;
            if !admit {
                tx.execute(
                    "UPDATE admission_requests SET state = ?2 WHERE device_pubkey = ?1",
                    params![target.as_slice(), REJECTED],
                )?;
                return Ok(Decided::Refused);
            }

            let vault =
                keys::vault_pubkey(tx)?.ok_or_else(|| fail(AdmissionError::NoDeviceList))?;
            let valid = device_list::valid_lists(&device_list::load_all(tx)?, &vault);
            let effective = device_list::effective(&valid)
                .ok_or_else(|| fail(AdmissionError::NoDeviceList))?
                .clone();
            if effective.list.device(&target).is_some() {
                delete_request(tx, &target)?;
                return Ok(Decided::AlreadyListed);
            }
            if effective.list.removes(&target) || request.verify().is_err() {
                delete_request(tx, &target)?;
                return Ok(Decided::Dropped);
            }
            let held = envelopes::held_keys(tx)?;
            let newest = held
                .first()
                .ok_or_else(|| fail(AdmissionError::NoContentKey))?;
            let mut devices = effective.list.devices.clone();
            devices.push(ListedDevice {
                device_pubkey: request.device,
                endpoint_id: request.endpoint,
                role: Role::Linked,
                vault_device_uuid: request.vault_device_uuid,
                name_sealed: content_keys::seal_name(newest, &request.device, &request.name),
                added_at: now_ms,
            });
            let next = DeviceList {
                generation: effective.list.generation + 1,
                base_list_hash: Some(effective.hash),
                issued_by: own.device_pubkey,
                issued_at: now_ms,
                devices,
                ..effective.list.clone()
            };
            next.check_structure().map_err(haex_crdt::Error::consumer)?;
            let signed =
                device_list::sign_list(next, &vault_secret).map_err(haex_crdt::Error::consumer)?;
            device_list::insert(tx, &signed)?;
            let wrapped = envelopes::envelopes_for(&held, &signed, &own, &request.device)?;
            envelopes::insert_envelopes(tx, &wrapped)?;
            delete_request(tx, &target)?;
            Ok(Decided::Admitted {
                list_generation: signed.list.generation,
                endpoint_id: request.endpoint,
            })
        })
        .map_err(unwrap_error)
}

/// Lets a copy of a main device enroll itself as a main device: the next
/// device list names it with its own device key, and its own key generation
/// envelopes are stored (FR-044). `false` when the copy's keys are not there
/// (nothing to enroll) or no content key is held.
pub fn enroll_as_main(
    tx: &mut CrdtTransaction<'_>,
    own: &DeviceKeys,
    vault_secret: &[u8; 32],
    effective: &SignedList,
    vault_device_uuid: Uuid,
    name: &str,
    now_ms: u64,
) -> haex_crdt::Result<bool> {
    let held = envelopes::held_keys(tx)?;
    let Some(newest) = held.first() else {
        log::warn!("sync: a copy of a main device holds no content key, it cannot enroll");
        return Ok(false);
    };
    let mut devices = effective.list.devices.clone();
    devices.push(ListedDevice {
        device_pubkey: own.device_pubkey,
        endpoint_id: own.endpoint_id,
        role: Role::Main,
        vault_device_uuid,
        name_sealed: content_keys::seal_name(newest, &own.device_pubkey, name),
        added_at: now_ms,
    });
    let next = DeviceList {
        generation: effective.list.generation + 1,
        base_list_hash: Some(effective.hash),
        issued_by: own.device_pubkey,
        issued_at: now_ms,
        devices,
        ..effective.list.clone()
    };
    next.check_structure().map_err(haex_crdt::Error::consumer)?;
    let signed = device_list::sign_list(next, vault_secret).map_err(haex_crdt::Error::consumer)?;
    device_list::insert(tx, &signed)?;
    let wrapped = envelopes::envelopes_for(&held, &signed, own, &own.device_pubkey)?;
    envelopes::insert_envelopes(tx, &wrapped)?;
    Ok(true)
}

/// This device's name for a request or a list entry: its alias, or the
/// computer's name while the alias is still the default.
pub fn own_name(q: &mut impl Query, vault_device_uuid: Uuid) -> haex_crdt::Result<String> {
    let alias: Option<Option<String>> = q.query_row(
        "SELECT alias FROM known_devices WHERE vault_device_uuid = ?1",
        params![vault_device_uuid.to_string()],
        |r| r.get(0),
    )?;
    let alias = alias
        .flatten()
        .map(|alias| alias.trim().to_string())
        .filter(|alias| !alias.is_empty() && alias != crate::sync::genesis::FALLBACK_DEVICE_NAME);
    Ok(alias
        .or_else(crate::hardware::hostname::suggested_alias)
        .unwrap_or_else(|| crate::sync::genesis::FALLBACK_DEVICE_NAME.to_string()))
}

/// Whether this device has to ask to be admitted: it is neither on the
/// effective list nor removed by it.
pub fn needs_request(effective: &SignedList, own: &[u8; 32]) -> bool {
    effective.list.device(own).is_none() && !effective.list.removes(own)
}

fn load_request(q: &mut impl Query, device: &[u8; 32]) -> haex_crdt::Result<Option<Request>> {
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

fn delete_request(tx: &mut CrdtTransaction<'_>, device: &[u8; 32]) -> haex_crdt::Result<()> {
    tx.execute(
        "DELETE FROM admission_requests WHERE device_pubkey = ?1",
        params![device.as_slice()],
    )?;
    Ok(())
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// An [`AdmissionError`] that crossed the storage transaction as a consumer
/// error, or the storage error itself.
fn unwrap_error(error: haex_crdt::Error) -> AdmissionError {
    match error {
        haex_crdt::Error::Consumer(inner) => match inner.downcast::<AdmissionError>() {
            Ok(admission) => *admission,
            Err(inner) => AdmissionError::Crdt(haex_crdt::Error::Consumer(inner)),
        },
        other => AdmissionError::Crdt(other),
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
