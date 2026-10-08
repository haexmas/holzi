//! The local media server and its HTTP (spec 044 FR-012, FR-013, FR-016, research R4).

pub mod http;
pub mod range;
pub mod server;

pub use server::MediaServer;
