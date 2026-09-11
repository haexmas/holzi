//! Provider seams holzi implements against `haex-crdt` 0.4.0.
//!
//! Contract: [`specs/001-frontend-onboarding/contracts/tauri-commands.md`](../../../../specs/001-frontend-onboarding/contracts/tauri-commands.md)
//! §"Vault identity and device model" and §"Provider implementations".
//!
//! Extraction into a standalone `haex-identity` crate stays deferred until
//! the trait surface has stabilised against a working slice (per plan §"Architektur
//! und Verantwortlichkeiten"). For now it lives as one Cargo module inside
//! `src-tauri`.

pub mod bootstrap;
pub mod installation;
pub mod migrations;

pub use bootstrap::HolziBootstrap;
pub use installation::{installation_id_path, read_or_mint_installation_uuid};
pub use migrations::{holzi_migration_source, HOLZI_TRIGGER_VERSION};

/// Nil UUID reserved as the sentinel row in `known_devices` representing
/// vault-scope for tables with a device-discriminating primary key. See
/// ADR-0001 (`docs/adr/0001-device-scoped-data-convention.md`).
pub const VAULT_SCOPE_UUID: uuid::Uuid = uuid::Uuid::nil();
