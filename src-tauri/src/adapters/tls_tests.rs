use std::fmt;

use rustls::CertificateError;

use super::*;

/// A transport error that names its cause, as reqwest and hyper do.
#[derive(Debug)]
struct Wrapped(Box<dyn Error + Send + Sync>);

impl fmt::Display for Wrapped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error sending request")
    }
}

impl Error for Wrapped {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

#[test]
fn a_refused_certificate_inside_an_io_error_is_found() {
    let refused = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        rustls::Error::InvalidCertificate(CertificateError::UnknownIssuer),
    );
    assert!(is_untrusted_certificate(&Wrapped(Box::new(refused))));
}

#[test]
fn other_transport_failures_are_not_about_the_certificate() {
    let refused = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    assert!(!is_untrusted_certificate(&Wrapped(Box::new(refused))));
    let protocol = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        rustls::Error::HandshakeNotComplete,
    );
    assert!(!is_untrusted_certificate(&Wrapped(Box::new(protocol))));
}
