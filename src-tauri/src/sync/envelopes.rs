//! The content key material a linking main device hands to a new device
//! (spec 024, user story 5, research R11): every generation it holds, and
//! the authorized envelopes that wrap them for the new device, built without
//! storing anything, since the new device's list is not published yet.

use haex_crdt::rusqlite::params;
use haex_crdt::CrdtTransaction;
use zeroize::Zeroizing;

use crate::storage::query::Query;
use crate::sync::content_keys::{
    fixed, sign, wrap, ContentKey, EnvelopeRecord, KeyError, StoredEnvelope,
};
use crate::sync::device_list::SignedList;
use crate::sync::keys::DeviceKeys;
use crate::sync::signing::Domain;

/// Every content key generation this device holds, highest first: what a
/// linking main device wraps for the new device.
pub fn held_keys(q: &mut impl Query) -> haex_crdt::Result<Vec<ContentKey>> {
    let held: Vec<(Vec<u8>, i64, Vec<u8>)> = q.query_map(
        "SELECT key_id, generation, key FROM vault_content_keys_no_sync \
         ORDER BY generation DESC",
        &[],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    held.into_iter()
        .map(|(key_id, generation, key)| {
            let key = Zeroizing::new(key);
            Ok(ContentKey {
                key_id: fixed(&key_id)?,
                generation: u64::try_from(generation).map_err(|_| KeyError::Malformed)?,
                key: Zeroizing::new(fixed(&key)?),
            })
        })
        .collect::<Result<_, KeyError>>()
        .map_err(Into::into)
}

/// The authorized envelopes that wrap each of `keys` for `recipient` under
/// `list`, without storing anything: a linking device sends them with the
/// transfer and stores them only when the new list is published.
pub fn envelopes_for(
    keys: &[ContentKey],
    list: &SignedList,
    issuer: &DeviceKeys,
    recipient: &[u8; 32],
) -> haex_crdt::Result<Vec<StoredEnvelope>> {
    keys.iter()
        .map(|key| {
            let envelope = wrap(key, issuer, recipient)?;
            let record = EnvelopeRecord {
                key_id: key.key_id,
                generation: key.generation,
                recipient: *recipient,
                sender: issuer.device_pubkey,
                envelope: &envelope,
                device_list_hash: list.hash,
            };
            let authorization = sign(Domain::KeyEnvelope, &record, &issuer.device_secret)?;
            Ok(StoredEnvelope {
                key_id: key.key_id.to_vec(),
                recipient: recipient.to_vec(),
                sender: issuer.device_pubkey.to_vec(),
                envelope,
                device_list_hash: list.hash.to_vec(),
                authorization: authorization.to_vec(),
            })
        })
        .collect()
}

/// Stores `envelopes` as they are, leaving rows that exist unchanged. They
/// only count once the list they reference is valid, and are checked then.
pub fn insert_envelopes(
    tx: &mut CrdtTransaction<'_>,
    envelopes: &[StoredEnvelope],
) -> haex_crdt::Result<()> {
    for e in envelopes {
        tx.execute(
            "INSERT OR IGNORE INTO vault_key_envelopes \
               (key_id, recipient, sender, envelope, device_list_hash, authorization) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                e.key_id,
                e.recipient,
                e.sender,
                e.envelope,
                e.device_list_hash,
                e.authorization
            ],
        )?;
    }
    Ok(())
}
