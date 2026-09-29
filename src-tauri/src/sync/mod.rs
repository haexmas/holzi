//! Own-device sync (spec 024): vault identity, device keys, device list,
//! content keys, the exchange of changes by progress per origin, and the
//! per-session sync service. The transport follows with user story 1.

pub mod change;
pub mod content_keys;
pub mod device_list;
pub mod events;
pub mod genesis;
pub mod inbound;
pub mod keys;
pub mod outbound;
pub mod progress;
pub mod replica;
mod service;
pub mod signing;

pub use service::SyncService;

#[cfg(test)]
pub(crate) mod test_support;
