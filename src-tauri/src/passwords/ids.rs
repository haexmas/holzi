//! Derived identifiers and name folding (spec 034, data-model.md §Abgeleitete Kennungen,
//! research R2).
//!
//! The password manager has no UNIQUE constraints: a UNIQUE conflict halts the sync of haex-crdt.
//! Tags, tag links and passkeys get their uniqueness from a UUIDv5 over their natural key instead,
//! so two devices that create the same tag independently produce the same row and merge.

use uuid::Uuid;

use unicode_normalization::UnicodeNormalization;

/// Namespace of `haex_passwords_tags.id`. Generated once; **never change** it, or the same tag
/// would get different ids on old and new devices.
pub const NS_TAG: Uuid = Uuid::from_u128(0x1318bf99_b22a_4792_9940_1e637fe41214);
/// Namespace of `haex_passwords_item_tags.id`. Never change.
pub const NS_ITEM_TAG: Uuid = Uuid::from_u128(0x2f0740cd_8b85_40a1_aea7_a5b02c6bf7b3);
/// Namespace of `haex_passwords_passkeys.id`. Never change.
pub const NS_PASSKEY: Uuid = Uuid::from_u128(0xe9e2f6e7_716b_43c4_85ba_a83fb47392c8);

/// The comparison form of a tag name: Unicode NFC, lower case, trimmed. "Work" and " work " are
/// one tag, and so are a precomposed `é` and `e` followed by a combining accent. An empty result
/// is not a valid tag name; the caller rejects it.
pub fn fold(name: &str) -> String {
    name.trim().nfc().collect::<String>().to_lowercase()
}

/// The id of the tag with this name: `UUIDv5(NS_TAG, fold(name))`.
pub fn tag_id(name: &str) -> Uuid {
    Uuid::new_v5(&NS_TAG, fold(name).as_bytes())
}

/// The id of the link between an item and a tag: `UUIDv5(NS_ITEM_TAG, "item_id:tag_id")`.
pub fn item_tag_id(item_id: &str, tag_id: &str) -> Uuid {
    Uuid::new_v5(&NS_ITEM_TAG, format!("{item_id}:{tag_id}").as_bytes())
}

/// The id of the passkey with this credential id: `UUIDv5(NS_PASSKEY, credential_id)`.
pub fn passkey_id(credential_id: &str) -> Uuid {
    Uuid::new_v5(&NS_PASSKEY, credential_id.as_bytes())
}
