//! SQL of migration `0029_agent_file_permissions` (spec 044, data-model.md, research R14): what each
//! agent may do with the files of the device and with each storage. Vault-wide and synced, so a
//! revoked grant holds on every own device. The id is derived from agent, kind and target (UUIDv5,
//! `files::permissions`), so two devices that answer the same question write the same row; like the
//! other synced tables it carries no UNIQUE constraint, which would halt the sync on a conflict.
//! New CRDT tables raise [`super::migrations::HOLZI_TRIGGER_VERSION`].

/// The migration `0029_agent_file_permissions`; registered in
/// [`super::migrations::holzi_migration_source`].
pub const AGENT_FILES_0029: &str = r#"CREATE TABLE agent_file_permissions (
  id TEXT PRIMARY KEY NOT NULL,
  agent_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  target TEXT NOT NULL,
  status TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);"#;
