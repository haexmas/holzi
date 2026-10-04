//! The report of an import (spec 034, FR-023, `contracts/import-mapping.md` §Bericht): numbers and a
//! list of the places the user has to rework by hand. A row names the entry, its folder path, the
//! kind of place and the field or file, never a value of a secret.

use super::{ImportModel, Problem};
use crate::passwords::model::{AttentionKind, AttentionRow, ImportReport};
use crate::passwords::ATTACHMENT_LIMIT_BYTES;

/// The stable name of a kind (the snake_case the window maps to a text).
pub fn kind_name(kind: AttentionKind) -> &'static str {
    match kind {
        AttentionKind::AttachmentTooLarge => "attachment_too_large",
        AttentionKind::AttachmentUnreadable => "attachment_unreadable",
        AttentionKind::PasskeyKeyUnreadable => "passkey_key_unreadable",
        AttentionKind::PasskeyPublicKeyMissing => "passkey_public_key_missing",
        AttentionKind::PasskeyDuplicate => "passkey_duplicate",
        AttentionKind::TotpInvalid => "totp_invalid",
        AttentionKind::IconNotMapped => "icon_not_mapped",
        AttentionKind::ValueNotStorable => "value_not_storable",
        AttentionKind::SourceSetting => "source_setting",
        AttentionKind::HistoryUnreadable => "history_unreadable",
        AttentionKind::GroupReparented => "group_reparented",
        AttentionKind::TagMerged => "tag_merged",
        AttentionKind::UnknownSourceData => "unknown_source_data",
    }
}

/// The folder path of a folder of the model, names joined by ` / `; empty at the top level.
pub fn folder_path(model: &ImportModel, group_ref: Option<&str>) -> String {
    let mut names = Vec::new();
    let mut current = group_ref;
    while let Some(reference) = current {
        let Some(group) = model.groups.iter().find(|g| g.reference == reference) else {
            break;
        };
        names.push(group.name.clone());
        current = group.parent_ref.as_deref();
        if names.len() > 64 {
            break;
        }
    }
    names.reverse();
    names.join(" / ")
}

/// Collects the numbers and rows of a run.
#[derive(Debug, Default)]
pub struct ReportBuilder {
    pub imported: u32,
    pub trashed: u32,
    pub history_states: u32,
    pub skipped_duplicates: u32,
    rows: Vec<AttentionRow>,
}

impl ReportBuilder {
    /// Adds a row for a problem at an entry (`title` empty and `folder_path` empty for the source).
    pub fn add(
        &mut self,
        item_id: Option<&str>,
        title: &str,
        folder_path: &str,
        problem: &Problem,
    ) {
        self.rows.push(AttentionRow {
            item_id: item_id.map(str::to_string),
            title: title.to_string(),
            folder_path: folder_path.to_string(),
            kind: problem.kind,
            field: problem.field.clone(),
            file_name: problem.file_name.clone(),
            size_mib: problem.size_mib,
        });
    }

    pub fn finish(self) -> ImportReport {
        ImportReport {
            imported: self.imported,
            trashed: self.trashed,
            history_states: self.history_states,
            skipped_duplicates: self.skipped_duplicates,
            needs_attention: self.rows,
        }
    }
}

/// The report as plain text for a file the user saves: the numbers, then one line per place to
/// rework. Titles, folder paths, kinds, field and file names and sizes; no value of a secret.
pub fn render_text(report: &ImportReport) -> String {
    let mut text = format!(
        "Import report\n\nImported entries: {}\nIn the trash: {}\nHistory states: {}\nSkipped duplicates: {}\n",
        report.imported, report.trashed, report.history_states, report.skipped_duplicates
    );
    if report.needs_attention.is_empty() {
        text.push_str("\nNothing needs your attention.\n");
        return text;
    }
    text.push_str("\nPlaces to rework by hand:\n");
    for row in &report.needs_attention {
        let mut line = format!("- [{}]", kind_name(row.kind));
        if !row.title.is_empty() {
            line.push_str(&format!(" {}", row.title));
        }
        if !row.folder_path.is_empty() {
            line.push_str(&format!(" (folder: {})", row.folder_path));
        }
        if let Some(field) = &row.field {
            line.push_str(&format!(", field: {field}"));
        }
        if let Some(file) = &row.file_name {
            line.push_str(&format!(", file: {file}"));
        }
        if let Some(size) = row.size_mib {
            line.push_str(&format!(
                ", size: {size:.2} MiB (limit {} MiB)",
                ATTACHMENT_LIMIT_BYTES / (1024 * 1024)
            ));
        }
        text.push_str(&line);
        text.push('\n');
    }
    text
}
