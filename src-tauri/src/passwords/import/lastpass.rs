//! LastPass CSV exports (spec 034, US7, `contracts/import-mapping.md` §LastPass). A row whose
//! address is `http://sn` is a secure note: its address stays empty. `extra` stays the note as it
//! is; on a secure note its `Key: Value` lines also become custom fields, so the structured data of
//! a note type (a card, a server) is searchable and copyable. Pure: `bytes → ImportModel`.

use super::{
    check_otp, csv, ensure_group_path, failed, non_empty, push_kv, push_tag, split_path,
    ImportItem, ImportModel,
};
use crate::error::Result;

const SECURE_NOTE_URL: &str = "http://sn";

/// Reads an export.
pub fn parse(bytes: &[u8]) -> Result<ImportModel> {
    let table = csv::parse(bytes)?;
    if !table.has_column("name") || !table.has_column("grouping") {
        return Err(failed("unsupported_format"));
    }
    let mut model = ImportModel::default();
    for row in &table.rows {
        let cell = |column: &str| table.cell(row, column).and_then(|v| non_empty(Some(v)));
        let url = cell("url");
        let secure_note = url.as_deref() == Some(SECURE_NOTE_URL);
        let mut item = ImportItem {
            title: cell("name"),
            username: cell("username"),
            password: cell("password"),
            url: if secure_note { None } else { url },
            note: cell("extra"),
            otp_raw: cell("totp"),
            ..ImportItem::default()
        };
        if secure_note
            || item
                .note
                .as_deref()
                .is_some_and(|n| n.starts_with("NoteType:"))
        {
            for line in item.note.clone().unwrap_or_default().lines() {
                let Some((key, value)) = line.split_once(':') else {
                    continue;
                };
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                let key = if key == "NoteType" {
                    "LastPass: Notiztyp"
                } else {
                    key
                };
                push_kv(&mut item.key_values, key, Some(value.trim()));
            }
        }
        if cell("fav").as_deref() == Some("1") {
            push_tag(&mut item.tags, "Favorit");
        }
        if let Some(grouping) = cell("grouping") {
            item.group_ref =
                ensure_group_path(&mut model.groups, &split_path(&grouping, &['/', '\\']));
        }
        check_otp(&mut item);
        model.items.push(item);
    }
    Ok(model)
}

#[cfg(test)]
#[path = "lastpass_tests.rs"]
mod tests;
