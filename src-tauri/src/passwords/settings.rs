//! The vault setting of the password manager (spec 034, FR-006): how long a copied value stays on
//! the clipboard. Stored through the preference commands as `passwords.clipboard_clear_seconds` in
//! the vault scope; the window saves it on selection.

use std::time::Duration;

use crate::error::Result;
use crate::storage::preferences::{self, PrefScope};
use crate::storage::query::Query;

/// The preference key.
pub const CLIPBOARD_CLEAR_KEY: &str = "passwords.clipboard_clear_seconds";
/// The choices of the window: 0 is off.
pub const CLIPBOARD_CLEAR_CHOICES: [u32; 5] = [0, 15, 30, 60, 120];
/// Used while nothing is stored.
pub const CLIPBOARD_CLEAR_DEFAULT: u32 = 30;

/// Whether `value` is an allowed stored value of [`CLIPBOARD_CLEAR_KEY`].
pub fn is_valid_clear_seconds(value: &str) -> bool {
    value.parse::<u32>().is_ok_and(|seconds| {
        CLIPBOARD_CLEAR_CHOICES.contains(&seconds) && seconds.to_string() == value
    })
}

/// The delay after which a copy is cleared; `None` when the setting is off. A stored value that is
/// not one of the choices (another device, an older build) reads as the default.
pub fn clear_after(q: &mut impl Query) -> Result<Option<Duration>> {
    let stored = preferences::get(q, PrefScope::Vault, CLIPBOARD_CLEAR_KEY)?;
    let seconds = stored
        .filter(|value| is_valid_clear_seconds(value))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(CLIPBOARD_CLEAR_DEFAULT);
    Ok((seconds > 0).then(|| Duration::from_secs(u64::from(seconds))))
}
