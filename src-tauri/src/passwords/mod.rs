//! The password manager (spec 034-password-manager): entries, folders, tags, history, attachments,
//! passkeys as data, the generator presets, the import and the access check for every caller.
//! Table and column names follow haex-vault (`haex_passwords_*`, data-model.md); every access goes
//! through [`service`] (added with the stories), never around it.

/// The attachment limit (FR-020): 25 MiB per attachment, checked before the file is read.
pub const ATTACHMENT_LIMIT_BYTES: u64 = 25 * 1024 * 1024;
/// How long a binary stays after the last known link vanished, so a link of another device that
/// has not arrived yet does not run into nothing (FR-022, research R4).
pub const ORPHAN_GRACE_DAYS: i64 = 7;
/// The id of the trash folder (FR-015): a folder row with a fixed id, `name` stays empty.
pub const TRASH_GROUP_ID: &str = "trash";

pub mod access;
#[cfg(test)]
mod access_tests;
pub mod binaries;
#[cfg(test)]
mod binaries_tests;
pub mod clipboard;
#[cfg(test)]
mod clipboard_tests;
pub mod clock;
#[cfg(test)]
mod clock_tests;
pub mod commands;
pub mod groups;
#[cfg(test)]
mod groups_tests;
pub mod ids;
#[cfg(test)]
mod ids_tests;
pub mod items;
#[cfg(test)]
mod items_tests;
pub mod maintenance;
pub mod model;
#[cfg(test)]
mod model_tests;
pub mod passkeys;
#[cfg(test)]
mod passkeys_tests;
pub mod presets;
#[cfg(test)]
mod presets_tests;
pub mod reveal;
#[cfg(test)]
mod reveal_tests;
pub mod service;
pub mod sets;
pub mod settings;
#[cfg(test)]
mod settings_tests;
pub mod snapshots;
#[cfg(test)]
mod snapshots_tests;
pub mod tags;
#[cfg(test)]
mod tags_tests;
#[cfg(test)]
pub(crate) mod test_support;
pub mod totp;
#[cfg(test)]
mod totp_tests;
pub mod trash;
#[cfg(test)]
mod trash_tests;
pub mod usage;
#[cfg(test)]
mod usage_tests;
