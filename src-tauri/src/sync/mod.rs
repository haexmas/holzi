//! Own-device sync (spec 024): vault identity, device keys, device list,
//! content keys and the per-session sync service. The transport follows with
//! user story 1.

pub mod content_keys;
pub mod device_list;
pub mod events;
pub mod genesis;
pub mod keys;
mod service;
pub mod signing;

pub use service::SyncService;

#[cfg(test)]
mod test_support;
