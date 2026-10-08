//! The real target of a path an extension names (spec 017, FR-047, research R19 step 1). The rule
//! itself lives in [`crate::files::local::resolve`], shared with the file browser and agents
//! (spec 044); this adapter keeps the bridge's error.

use std::path::{Path, PathBuf};

use crate::extensions::error::{BridgeError, ExtensionErrorCode};

/// The canonical target of the absolute `path`; see [`crate::files::local::resolve`].
pub fn resolve(path: &str) -> Result<PathBuf, BridgeError> {
    crate::files::local::resolve(Path::new(path))
        .map_err(|error| BridgeError::new(ExtensionErrorCode::Validation, error.message))
}
