//! Types of the references between entries (spec 036, data-model.md, `contracts/references.md`):
//! what the window learns about the placeholders of a text, never a resolved value.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The value a placeholder points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum RefMarkKind {
    Username,
    Password,
    Extra,
}

/// Whether a placeholder resolves for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum RefStatus {
    Ok,
    Missing,
    Cycle,
    TooDeep,
}

/// One placeholder in a text: its place (UTF-16 offsets, as the window counts), its source and
/// whether it resolves. No value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct RefMark {
    pub start: u32,
    pub end: u32,
    pub source_item_id: String,
    /// The title of the source, `None` when it does not exist (or has none).
    pub source_title: Option<String>,
    pub kind: RefMarkKind,
    /// The key of the custom field for `extra`.
    pub key: Option<String>,
    pub status: RefStatus,
}

/// The placeholders of the custom field `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct KeyValueReferences {
    pub id: String,
    pub marks: Vec<RefMark>,
}

/// The placeholders in the fields of an entry (`ItemDetail.references`): the window shows marks
/// instead of the raw text and does not need to load the password for them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ItemReferences {
    pub username: Vec<RefMark>,
    pub password: Vec<RefMark>,
    pub url: Vec<RefMark>,
    pub note: Vec<RefMark>,
    /// Only the custom fields that hold a placeholder.
    pub key_values: Vec<KeyValueReferences>,
}

/// How many other entries point at an entry (FR-048), before it is deleted for good.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ReferenceUsage {
    pub item_id: String,
    /// Entries with a placeholder on this one in a text field.
    pub target_items: u32,
    /// Entries that use one of its passkeys through a link (stage 4).
    pub passkey_links: u32,
}
