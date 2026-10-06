//! Which endpoints holzi talks to (spec 038 FR-017, research R8).
//!
//! [`check_endpoint`] looks at the address as typed: only `http` and `https`, no user name or
//! password in it, and for an endpoint an extension proposed neither `http`, nor `localhost`, nor
//! an IP address it may not reach. [`pin`] runs before every call: it resolves the host once,
//! checks every address, and hands back the addresses the request must go to, so a name that
//! points elsewhere a moment later (DNS rebinding) reaches nothing new.
//!
//! - Never: link-local (`169.254.0.0/16` with the metadata service of a cloud, `fe80::/10`),
//!   unspecified, multicast and broadcast addresses, also as IPv4-mapped IPv6.
//! - Only for an endpoint the user typed: loopback and private addresses (RFC 1918, RFC 4193).
//! - `http` only when every address is one of those, so only for the user's own endpoints.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use async_trait::async_trait;
use reqwest::Url;
use url::Host;

use super::{EndpointOrigin, Location, ProviderKind};

/// Why an endpoint is refused; neither variant carries the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    /// Not an `http`/`https` address with a host, or it carries a user name, a password, a query
    /// or a fragment.
    #[error("invalid endpoint")]
    Invalid,
    /// The address is one holzi does not reach for this endpoint.
    #[error("endpoint not allowed")]
    NotAllowed,
}

/// What an address is, for the rules above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Forbidden,
    Local,
    Public,
}

/// The class of `ip`; an IPv4-mapped IPv6 address counts as its IPv4 address.
pub fn classify(ip: IpAddr) -> Class {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => classify_v4(v4),
            None => classify_v6(v6),
        },
    }
}

/// Classifies IPv4 addresses as forbidden, local or public for endpoint validation.
fn classify_v4(ip: Ipv4Addr) -> Class {
    if ip.is_link_local()
        || ip.is_unspecified()
        || ip.octets()[0] == 0
        || ip.is_multicast()
        || ip.is_broadcast()
    {
        Class::Forbidden
    } else if ip.is_loopback() || ip.is_private() {
        Class::Local
    } else {
        Class::Public
    }
}

/// Classifies an IPv6 address after the caller has handled IPv4-mapped addresses.
fn classify_v6(ip: Ipv6Addr) -> Class {
    let first = ip.segments()[0];
    if ip.is_unspecified() || ip.is_multicast() || first & 0xffc0 == 0xfe80 {
        Class::Forbidden
    } else if ip.is_loopback() || first & 0xfe00 == 0xfc00 {
        Class::Local
    } else {
        Class::Public
    }
}

/// Whether holzi may connect to `ip` for an endpoint of `origin` over `https` or `http`.
pub fn allowed(ip: IpAddr, origin: EndpointOrigin, https: bool) -> bool {
    match classify(ip) {
        Class::Forbidden => false,
        Class::Local => origin == EndpointOrigin::User,
        Class::Public => https,
    }
}

/// Recognizes localhost and its subdomains regardless of case or a trailing dot.
fn is_localhost(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name == "localhost" || name.ends_with(".localhost")
}

/// Checks an endpoint as typed (see the module). For a host name the addresses are checked by
/// [`pin`] before every call.
pub fn check_endpoint(endpoint: &str, origin: EndpointOrigin) -> Result<Url, AddressError> {
    let url = Url::parse(endpoint.trim()).map_err(|_| AddressError::Invalid)?;
    let https = match url.scheme() {
        "https" => true,
        "http" => false,
        _ => return Err(AddressError::Invalid),
    };
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AddressError::Invalid);
    }
    let from_extension = origin == EndpointOrigin::Extension;
    if from_extension && !https {
        return Err(AddressError::NotAllowed);
    }
    match url.host().ok_or(AddressError::Invalid)? {
        Host::Domain(name) if from_extension && is_localhost(name) => {
            return Err(AddressError::NotAllowed)
        }
        Host::Domain(_) => {}
        Host::Ipv4(ip) if !allowed(IpAddr::V4(ip), origin, https) => {
            return Err(AddressError::NotAllowed)
        }
        Host::Ipv6(ip) if !allowed(IpAddr::V6(ip), origin, https) => {
            return Err(AddressError::NotAllowed)
        }
        Host::Ipv4(_) | Host::Ipv6(_) => {}
    }
    Ok(url)
}

/// The address a connection talks to: its endpoint, or for AWS without one the regional one.
pub fn endpoint_url(location: &Location) -> Result<Url, AddressError> {
    if location.provider_kind == ProviderKind::Aws && location.endpoint.trim().is_empty() {
        let region = &location.region;
        if region.is_empty()
            || !region
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(AddressError::Invalid);
        }
        return Url::parse(&format!("https://s3.{region}.amazonaws.com"))
            .map_err(|_| AddressError::Invalid);
    }
    check_endpoint(&location.endpoint, location.endpoint_origin)
}

/// Whether the endpoint sends without encryption; the settings and the dialog mark it.
pub fn is_insecure(url: &Url) -> bool {
    url.scheme() == "http"
}

/// Turns a host name into addresses; the system resolver in holzi, a fixed table in tests.
#[async_trait]
pub trait Resolver: Send + Sync {
    /// Resolves `host` and `port` to candidate IP addresses, or returns a lookup error.
    async fn lookup(&self, host: &str, port: u16) -> io::Result<Vec<IpAddr>>;
}

/// The resolver of the operating system.
pub struct SystemResolver;

#[async_trait]
impl Resolver for SystemResolver {
    /// Returns the IP addresses found by the operating system for `host` and `port`.
    async fn lookup(&self, host: &str, port: u16) -> io::Result<Vec<IpAddr>> {
        Ok(tokio::net::lookup_host((host, port))
            .await?
            .map(|addr| addr.ip())
            .collect())
    }
}

/// The checked addresses of one request's host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// The host name the request keeps for TLS and the signature; `None` for an IP address.
    pub host: Option<String>,
    pub addrs: Vec<SocketAddr>,
}

/// Resolves the host of `url` once and checks every address (see the module). One address that is
/// not allowed refuses the whole request.
pub async fn pin(
    url: &Url,
    origin: EndpointOrigin,
    resolver: &dyn Resolver,
) -> Result<Pinned, AddressError> {
    let https = match url.scheme() {
        "https" => true,
        "http" => false,
        _ => return Err(AddressError::Invalid),
    };
    let port = url.port_or_known_default().ok_or(AddressError::Invalid)?;
    let (host, ips) = match url.host().ok_or(AddressError::Invalid)? {
        Host::Ipv4(ip) => (None, vec![IpAddr::V4(ip)]),
        Host::Ipv6(ip) => (None, vec![IpAddr::V6(ip)]),
        Host::Domain(name) => {
            if origin == EndpointOrigin::Extension && is_localhost(name) {
                return Err(AddressError::NotAllowed);
            }
            let ips = resolver
                .lookup(name, port)
                .await
                .map_err(|_| AddressError::NotAllowed)?;
            (Some(name.to_owned()), ips)
        }
    };
    if ips.is_empty() || !ips.iter().all(|ip| allowed(*ip, origin, https)) {
        return Err(AddressError::NotAllowed);
    }
    Ok(Pinned {
        host,
        addrs: ips
            .into_iter()
            .map(|ip| SocketAddr::new(ip, port))
            .collect(),
    })
}
