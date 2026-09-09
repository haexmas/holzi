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
pub use migrations::holzi_migration_source;
