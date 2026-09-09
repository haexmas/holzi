//! Instance lifecycle: create, open, close, list.
//!
//! Contract: `specs/001-frontend-onboarding/contracts/tauri-commands.md`.
//! One file per command mirrors haex-vault's convention.

pub mod paths;
pub mod startup;

mod close;
mod create;
mod events;
mod info;
mod list;
mod open;

pub use close::close_instance;
pub use create::create_instance;
pub use info::InstanceInfo;
pub use list::list_instances;
pub use open::open_instance;
pub use startup::cleanup_orphans_on_startup;

#[cfg(test)]
mod paths_tests;
#[cfg(test)]
mod startup_tests;
