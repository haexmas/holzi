//! The real target of a path an extension names (spec 017, FR-047, research R19 step 1): every
//! check runs on it, never on the text the extension sent, so `..`, a symbolic link or another
//! spelling cannot leave a granted folder.

use std::path::{Component, Path, PathBuf};

use crate::extensions::error::{BridgeError, ExtensionErrorCode};

fn invalid(message: &str) -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Validation, message)
}

/// The canonical target of the absolute `path`. A target that does not exist yet (a file to
/// write, a folder to create) resolves through its nearest existing ancestor; the rest is appended
/// and may hold no `.` or `..`. A part that is there but does not resolve is a broken or looping
/// link: writing through it would reach a target nobody checked, so it is refused.
pub fn resolve(path: &str) -> Result<PathBuf, BridgeError> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(invalid("path must be absolute"));
    }
    if let Ok(real) = std::fs::canonicalize(path) {
        return Ok(real);
    }
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    let mut ancestor = path;
    loop {
        if std::fs::symlink_metadata(ancestor).is_ok() {
            return Err(invalid("path holds a broken link"));
        }
        let parent = ancestor
            .parent()
            .ok_or_else(|| invalid("path has no existing ancestor"))?;
        match ancestor.components().next_back() {
            Some(Component::Normal(name)) => rest.push(name),
            _ => return Err(invalid("path must not contain . or ..")),
        }
        if let Ok(real) = std::fs::canonicalize(parent) {
            let mut resolved = real;
            for name in rest.into_iter().rev() {
                resolved.push(name);
            }
            return Ok(resolved);
        }
        ancestor = parent;
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
