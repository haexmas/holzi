//! File attachments a user adds to the message being composed (spec
//! 011-composer-toolbar-parity). Classification is by file extension and
//! byte size only — never by parsing file contents, so a corrupt or
//! disguised file fails at the provider's own decode step, not here.
//!
//! Two separate concerns, kept apart because they answer different
//! questions (data-model.md "Message Attachment"): [`classify_attachment`]
//! ("is this file even attachable at all" — FR-016, independent of any
//! backend) and [`usability_for`] ("can the *currently selected*
//! model/backend use it" — FR-015, independent of the file itself).

use std::path::Path;

use serde::Serialize;

use crate::adapters::{Attachment, AttachmentKind};
use crate::error::{HolziError, Result};
use crate::storage::providers::ProviderKind;

/// Anthropic's own published per-content-type limits (research.md §4) —
/// reused rather than inventing holzi-specific numbers.
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 5 * 1024 * 1024;

fn max_bytes_for(kind: &AttachmentKind) -> u64 {
    match kind {
        AttachmentKind::Image => MAX_IMAGE_BYTES,
        AttachmentKind::Document => MAX_DOCUMENT_BYTES,
        AttachmentKind::Text => MAX_TEXT_BYTES,
    }
}

/// Extension-based kind + media-type detection. `None` for anything not in
/// this list — an unrecognized extension is an unsupported type (FR-016),
/// not a guess.
fn kind_and_media_type(path: &Path) -> Option<(AttachmentKind, &'static str)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => (AttachmentKind::Image, "image/png"),
        "jpg" | "jpeg" => (AttachmentKind::Image, "image/jpeg"),
        "webp" => (AttachmentKind::Image, "image/webp"),
        "gif" => (AttachmentKind::Image, "image/gif"),
        "pdf" => (AttachmentKind::Document, "application/pdf"),
        "txt" => (AttachmentKind::Text, "text/plain"),
        "md" => (AttachmentKind::Text, "text/markdown"),
        _ => return None,
    })
}

fn human_bytes(bytes: u64) -> String {
    format!("{} MB", bytes / (1024 * 1024))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub name: String,
    pub size_bytes: u64,
    /// `None` when the file's extension isn't a supported attachment type
    /// at all (`usable` is then always `false`).
    pub kind: Option<AttachmentKindWire>,
    pub usable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKindWire {
    Image,
    Document,
    Text,
}

impl From<&AttachmentKind> for AttachmentKindWire {
    fn from(kind: &AttachmentKind) -> Self {
        match kind {
            AttachmentKind::Image => AttachmentKindWire::Image,
            AttachmentKind::Document => AttachmentKindWire::Document,
            AttachmentKind::Text => AttachmentKindWire::Text,
        }
    }
}

impl From<AttachmentKindWire> for AttachmentKind {
    fn from(kind: AttachmentKindWire) -> Self {
        match kind {
            AttachmentKindWire::Image => AttachmentKind::Image,
            AttachmentKindWire::Document => AttachmentKind::Document,
            AttachmentKindWire::Text => AttachmentKind::Text,
        }
    }
}

/// "Is this file even attachable at all" — type + size, independent of any
/// backend (FR-016). Errors only when the file cannot be read/stat'd at
/// all (contracts/tauri-commands.md `inspect_attachment`); an oversized or
/// unsupported-type file still resolves normally with `usable: false`.
pub fn classify_attachment(path: &Path) -> Result<AttachmentInfo> {
    let metadata = std::fs::metadata(path).map_err(|e| HolziError::InvalidInput {
        reason: format!("cannot read attachment {}: {e}", path.display()),
    })?;
    let size_bytes = metadata.len();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let Some((kind, _media_type)) = kind_and_media_type(path) else {
        return Ok(AttachmentInfo {
            name,
            size_bytes,
            kind: None,
            usable: false,
            reason: Some("unsupported file type".to_string()),
        });
    };
    let cap = max_bytes_for(&kind);
    if size_bytes > cap {
        return Ok(AttachmentInfo {
            name,
            size_bytes,
            kind: Some((&kind).into()),
            usable: false,
            reason: Some(format!(
                "exceeds the {} limit for this file type",
                human_bytes(cap)
            )),
        });
    }
    Ok(AttachmentInfo {
        name,
        size_bytes,
        kind: Some((&kind).into()),
        usable: true,
        reason: None,
    })
}

/// "Can the currently selected model/backend use a file of this kind" —
/// FR-015, independent of the file itself. `kind` is accepted for a
/// forward-compatible signature (a future backend might support only some
/// kinds) but every backend that supports attachments at all supports all
/// three kinds today (data-model.md).
pub fn usability_for(
    _kind: &AttachmentKind,
    provider_kind: ProviderKind,
    adapter: Option<&str>,
) -> bool {
    matches!(
        (provider_kind, adapter),
        (ProviderKind::ApiKey, Some("anthropic")) | (ProviderKind::CliDelegate, Some("claude"))
    )
}

/// Re-stats and re-reads the file at send time (FR-018 — a file can vanish
/// or change between attach and send). Errors here mean the caller must
/// exclude this one attachment from the send, not fail the whole message.
pub fn read_attachment_content(path: &Path) -> Result<Attachment> {
    let (kind, media_type) = kind_and_media_type(path).ok_or_else(|| HolziError::InvalidInput {
        reason: format!("unsupported attachment type: {}", path.display()),
    })?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| HolziError::InvalidInput {
        reason: format!("failed to read attachment {}: {e}", path.display()),
    })?;
    Ok(Attachment {
        name,
        kind,
        media_type: media_type.to_string(),
        bytes,
    })
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
