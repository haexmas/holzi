//! CRDT-aware write helpers.
//!
//! Etappe-0 finding #2: every INSERT/UPDATE on a CRDT-tracked table must set
//! `haex_hlc_no_sync = current_hlc()` in the SET/VALUES clause. Without it,
//! the row-level HLC stays NULL, the trigger skips the column-HLC map, and
//! the write is invisible to the sync scanner (LWW/sync silently breaks).
//!
//! This module makes it impossible to forget the convention: writes go
//! through typed helpers that always append the HLC column.
//!
//! Bootstrap-time inserts (`HolziBootstrap`) are the deliberate exception —
//! the crate hasn't initialised HLC yet inside the bootstrap transaction, so
//! `current_hlc()` would fail. Those rows carry NULL row-level HLC and stay
//! sync-invisible until a later UPDATE fires the trigger. See contract
//! §"Vault identity and device model".

pub mod chat_messages;
#[cfg(test)]
mod chat_messages_tests;
pub mod chat_threads;
pub mod known_devices;
pub mod models;
pub mod preferences;
pub mod preferences_commands;
#[cfg(test)]
pub mod preferences_commands_tests;
#[cfg(test)]
pub mod preferences_tests;
pub mod providers;
