//! Telling an untrusted certificate apart from other transport failures (spec 043 FR-024): the
//! connection to a provider whose certificate the system does not trust is refused, and the
//! person learns why instead of reading a generic network error. Retrying would not help.

use std::error::Error;

/// Whether `error` or anything it was caused by is rustls refusing the server's certificate. An
/// `io::Error` hides what it wraps from `source()`, so its inner error is looked into as well.
pub fn is_untrusted_certificate(error: &(dyn Error + 'static)) -> bool {
    let mut next = Some(error);
    while let Some(current) = next {
        if let Some(rustls::Error::InvalidCertificate(_)) = current.downcast_ref::<rustls::Error>()
        {
            return true;
        }
        if let Some(inner) = current
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
        {
            if is_untrusted_certificate(inner) {
                return true;
            }
        }
        next = current.source();
    }
    false
}

#[cfg(test)]
#[path = "tls_tests.rs"]
mod tests;
