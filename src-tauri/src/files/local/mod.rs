//! Files of this device (spec 044, research R3): resolving paths, holzi's own places and the known
//! places. Extensions (`extensions::fs`), the file browser and agents share these rules.

pub mod places;
pub mod resolve;
#[cfg(not(target_os = "ios"))]
pub mod watch;

pub use places::{known_places, OwnPlaces, Place};
pub use resolve::resolve;
