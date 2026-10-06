//! The endpoint rules of research R8: a table of typed endpoints for both origins, and pinning with
//! an injected resolver (a name that resolves to a local address, DNS rebinding, a mixed answer).

use std::io;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use reqwest::Url;

use super::address::{check_endpoint, endpoint_url, is_insecure, pin, AddressError, Resolver};
use super::{Addressing, EndpointOrigin, Location, ProviderKind};

use EndpointOrigin::{Extension, User};

#[test]
fn typed_endpoints_follow_the_rules_for_their_origin() {
    let ok = |e: &str, o| check_endpoint(e, o).map(|_| ());
    let cases: &[(&str, EndpointOrigin, Result<(), AddressError>)] = &[
        ("https://s3.example.com", Extension, Ok(())),
        ("https://s3.example.com", User, Ok(())),
        (
            "http://203.0.113.7:9000",
            User,
            Err(AddressError::NotAllowed),
        ),
        ("http://192.168.1.5:9000", User, Ok(())),
        (
            "http://192.168.1.5:9000",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        (
            "https://192.168.1.5:9000",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        ("http://127.0.0.1:9000", User, Ok(())),
        ("http://localhost:9000", User, Ok(())),
        (
            "https://localhost:9000",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        (
            "https://minio.localhost",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        (
            "http://s3.example.com",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        (
            "http://169.254.169.254",
            User,
            Err(AddressError::NotAllowed),
        ),
        (
            "http://169.254.169.254",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        ("https://[fe80::1]", User, Err(AddressError::NotAllowed)),
        (
            "https://[fe80::1]",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        (
            "https://[::ffff:169.254.169.254]",
            User,
            Err(AddressError::NotAllowed),
        ),
        (
            "https://[::ffff:169.254.169.254]",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        (
            "https://[::ffff:10.0.0.1]",
            Extension,
            Err(AddressError::NotAllowed),
        ),
        ("http://[fd00::5]:9000", User, Ok(())),
        ("https://0.0.0.0", User, Err(AddressError::NotAllowed)),
        ("https://224.0.0.1", User, Err(AddressError::NotAllowed)),
        (
            "https://255.255.255.255",
            User,
            Err(AddressError::NotAllowed),
        ),
        ("ftp://s3.example.com", User, Err(AddressError::Invalid)),
        (
            "https://key:secret@s3.example.com",
            User,
            Err(AddressError::Invalid),
        ),
        (
            "https://key@s3.example.com",
            Extension,
            Err(AddressError::Invalid),
        ),
        (
            "https://s3.example.com?x=1",
            User,
            Err(AddressError::Invalid),
        ),
        ("not an address", User, Err(AddressError::Invalid)),
    ];
    for (endpoint, origin, expected) in cases {
        assert_eq!(
            &ok(endpoint, *origin),
            expected,
            "{endpoint} for {origin:?}"
        );
    }
}

#[test]
fn an_http_endpoint_of_the_user_is_marked_insecure() {
    let url = check_endpoint("http://192.168.1.5:9000", User).expect("allowed");
    assert!(is_insecure(&url));
    let url = check_endpoint("https://s3.example.com", User).expect("allowed");
    assert!(!is_insecure(&url));
}

fn aws(region: &str) -> Location {
    Location {
        provider_kind: ProviderKind::Aws,
        endpoint: String::new(),
        endpoint_origin: User,
        region: region.to_owned(),
        addressing: Addressing::Virtual,
        bucket: "b".to_owned(),
    }
}

#[test]
fn aws_without_an_endpoint_uses_the_regional_address() {
    assert_eq!(
        endpoint_url(&aws("eu-central-1")).expect("url").as_str(),
        "https://s3.eu-central-1.amazonaws.com/"
    );
    assert_eq!(
        endpoint_url(&aws("eu/../x")),
        Err(AddressError::Invalid),
        "the region becomes part of a host name"
    );
}

/// Answers from a list, one answer per lookup, and counts the lookups.
struct Answers {
    answers: Mutex<Vec<Vec<IpAddr>>>,
    lookups: AtomicUsize,
}

impl Answers {
    fn new(answers: &[&[&str]]) -> Self {
        Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .rev()
                    .map(|a| a.iter().map(|ip| ip.parse().expect("ip")).collect())
                    .collect(),
            ),
            lookups: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Resolver for Answers {
    async fn lookup(&self, _host: &str, _port: u16) -> io::Result<Vec<IpAddr>> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        self.answers
            .lock()
            .expect("lock")
            .pop()
            .ok_or_else(|| io::Error::other("no answer"))
    }
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

#[tokio::test]
async fn a_name_that_resolves_to_a_local_address_is_refused_for_an_extension() {
    let resolver = Answers::new(&[&["127.0.0.1"], &["127.0.0.1"]]);
    let target = url("https://storage.example.com");
    assert_eq!(
        pin(&target, Extension, &resolver).await,
        Err(AddressError::NotAllowed)
    );
    let pinned = pin(&target, User, &resolver).await.expect("the user's own");
    assert_eq!(pinned.host.as_deref(), Some("storage.example.com"));
    assert_eq!(pinned.addrs, vec!["127.0.0.1:443".parse().expect("addr")]);
}

#[tokio::test]
async fn the_request_goes_to_the_address_checked_once_not_to_a_later_answer() {
    let resolver = Answers::new(&[&["203.0.113.7"], &["169.254.169.254"]]);
    let pinned = pin(&url("https://s3.example.com:9000"), Extension, &resolver)
        .await
        .expect("a public address");
    assert_eq!(
        pinned.addrs,
        vec!["203.0.113.7:9000".parse().expect("addr")]
    );
    assert_eq!(
        resolver.lookups.load(Ordering::SeqCst),
        1,
        "resolved once; the second answer (DNS rebinding) is never asked for"
    );
}

#[tokio::test]
async fn one_disallowed_address_in_an_answer_refuses_the_request() {
    let resolver = Answers::new(&[&["203.0.113.7", "169.254.169.254"]]);
    assert_eq!(
        pin(&url("https://s3.example.com"), User, &resolver).await,
        Err(AddressError::NotAllowed)
    );
}

#[tokio::test]
async fn http_to_a_public_address_is_refused_also_for_the_user() {
    let resolver = Answers::new(&[&["203.0.113.7"]]);
    assert_eq!(
        pin(&url("http://s3.example.com"), User, &resolver).await,
        Err(AddressError::NotAllowed)
    );
}

#[tokio::test]
async fn an_ip_address_is_checked_without_a_lookup() {
    let resolver = Answers::new(&[]);
    let pinned = pin(&url("http://192.168.1.5:9000"), User, &resolver)
        .await
        .expect("the user's own");
    assert_eq!(pinned.host, None);
    assert_eq!(resolver.lookups.load(Ordering::SeqCst), 0);
    assert_eq!(
        pin(&url("https://[::ffff:169.254.169.254]"), User, &resolver).await,
        Err(AddressError::NotAllowed)
    );
}

#[tokio::test]
async fn a_name_that_does_not_resolve_is_refused() {
    let resolver = Answers::new(&[&[]]);
    assert_eq!(
        pin(&url("https://nowhere.example.com"), User, &resolver).await,
        Err(AddressError::NotAllowed)
    );
}
