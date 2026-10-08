//! OS-hostname detection for the onboarding wizard's alias placeholder.
//!
//! Backend returns the raw `Option<String>` from `sysinfo`; the frontend
//! applies the localised fallback via `$t('onboarding.alias.defaultPlaceholder')`
//! when it comes back `None` (spec 002 §FR-020 i18n boundary).
//!
//! On Android the host name is "localhost"; the platform names the phone instead (spec 043,
//! research R10), told once at startup through [`use_platform_name`].

use std::sync::OnceLock;

use sysinfo::System;

/// The device name the platform reported, where the host name says nothing (Android).
static PLATFORM_NAME: OnceLock<String> = OnceLock::new();

/// Records the device name the platform reports; the first usable one stays.
pub fn use_platform_name(name: Option<String>) {
    if let Some(name) = name.and_then(usable) {
        let _ = PLATFORM_NAME.set(name);
    }
}

/// Returns the platform's device name, else the current host's name from `sysinfo`, or `None` when
/// neither names the device. Never blocks meaningfully — `sysinfo` reads it from the OS on demand
/// without a full system probe.
pub fn suggested_alias() -> Option<String> {
    choose(PLATFORM_NAME.get().cloned(), System::host_name())
}

/// The platform's name wins over the host name; neither counts when empty or "localhost".
fn choose(platform: Option<String>, host: Option<String>) -> Option<String> {
    platform.and_then(usable).or_else(|| host.and_then(usable))
}

fn usable(name: String) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("localhost") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
#[path = "hostname_tests.rs"]
mod tests;
