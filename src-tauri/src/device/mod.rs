//! Device-scoped commands. Holds the current-device metadata surface
//! (installation, vault-device UUID, alias, OS hostname) and the alias
//! rename entry point. See spec 002 §"Onboarding" and
//! [ADR-0001](../../../docs/adr/0001-device-scoped-data-convention.md).

pub mod commands;
