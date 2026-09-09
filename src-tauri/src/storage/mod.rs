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

pub mod known_devices;
