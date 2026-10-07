//! Certificate checks against the trust store of the device (spec 043 FR-024, research R2).
//!
//! On the desktop `rustls-platform-verifier` needs nothing. On Android it asks the system's
//! `X509TrustManager` through JNI and has to be given the VM and the application context once,
//! before the first TLS connection; without that the first check panics.

#[cfg(target_os = "android")]
mod android;

/// Prepares certificate checks for this process. Call it once at startup, before any network
/// service runs; calling it again does nothing.
pub fn init_platform_verifier() -> Result<(), InitError> {
    #[cfg(target_os = "android")]
    android::init()?;
    Ok(())
}

/// The platform verifier could not be prepared; no TLS connection would work.
#[derive(Debug, thiserror::Error)]
#[error("preparing certificate checks failed: {0}")]
pub struct InitError(String);

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
