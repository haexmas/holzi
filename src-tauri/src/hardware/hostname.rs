//! OS-hostname detection for the onboarding wizard's alias placeholder.
//!
//! Backend returns the raw `Option<String>` from `sysinfo`; the frontend
//! applies the localised fallback via `$t('onboarding.alias.defaultPlaceholder')`
//! when it comes back `None` (spec 002 §FR-020 i18n boundary).

use sysinfo::System;

/// Returns the current host's name from `sysinfo`, or `None` when the
/// OS could not report one. Never blocks meaningfully — `sysinfo`
/// reads it from the OS on demand without a full system probe.
pub fn suggested_alias() -> Option<String> {
    System::host_name().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
