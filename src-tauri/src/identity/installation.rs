//! Installation UUID — one random UUID per holzi installation on a host,
//! stored in `<AppLocalData>/installation-id`. Never leaves the device.
//!
//! Contract: `tauri-commands.md` §"Vault identity and device model" — the
//! second bullet ("Installation UUID"). Kept as a file outside any vault so
//! that a copied `.db` reopened by the same installation can look up its
//! existing `known_devices` row.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use uuid::Uuid;

/// The bare filename holzi writes under `<AppLocalData>/`.
pub const INSTALLATION_ID_FILENAME: &str = "installation-id";

/// Resolves the installation-id path from an `AppLocalData` root. Kept
/// separate from the Tauri path resolver so tests can point at a temp dir.
pub fn installation_id_path(app_local_data: &Path) -> PathBuf {
    app_local_data.join(INSTALLATION_ID_FILENAME)
}

/// Reads the existing installation UUID from `path`, or mints and fsyncs a
/// fresh one if the file does not exist. Idempotent — repeated calls always
/// return the same UUID.
pub fn read_or_mint_installation_uuid(path: &Path) -> std::io::Result<Uuid> {
    match fs::read_to_string(path) {
        Ok(s) => Uuid::parse_str(s.trim())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => mint_and_fsync(path),
        Err(e) => Err(e),
    }
}

fn mint_and_fsync(path: &Path) -> std::io::Result<Uuid> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let fresh = Uuid::new_v4();

    let mut opts = OpenOptions::new();
    opts.create_new(true).write(true);
    #[cfg(unix)]
    opts.mode(0o600);

    let mut f = opts.open(path)?;
    f.write_all(fresh.to_string().as_bytes())?;
    f.sync_all()?;
    Ok(fresh)
}
