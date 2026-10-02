//! Targets of permissions and how a request matches them (contracts/permissions.md §Arten).
//!
//! Every kind has its own target form. A path matches by whole path components and a domain at a
//! label boundary, so `/a/docs` never covers `/a/docs-private` and `example.org` never covers
//! `badexample.org`. A request target is always the resolved one (a canonical path, a parsed URL);
//! resolving is the caller's job.

use std::path::{Component, Path, PathBuf};

use super::model::PermissionKind;
use crate::extensions::ids::{ExtensionName, ExtensionTable, PublicKey, TablePrefix};
use crate::passwords::ids::fold;

/// The target of a stored permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `*`: everything of this kind (not for `database`).
    Any,
    /// All tables of one extension: `<publicKey>__<name>__*`.
    ExtensionTables(TablePrefix),
    /// One table of one extension.
    ExtensionTable(ExtensionTable),
    /// A file, or a folder with everything below it.
    Path(PathBuf),
    /// `scheme://host[:port]/path[*]`.
    Url(UrlPattern),
    /// A domain without scheme: the domain itself and its subdomains, or with `*.` only the
    /// subdomains.
    Domain(HostPattern),
    /// A mail server, on one port or on all.
    MailServer { host: String, port: Option<u16> },
    /// A program, by its canonical path.
    Program(PathBuf),
    /// A tag of the password manager, compared in its folded form.
    Tag(String),
    /// A remote storage connection.
    StorageId(String),
}

/// How a host part of a target matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    /// Exactly this host.
    Exact(String),
    /// Only subdomains of this domain (`*.example.org`).
    Subdomains(String),
    /// The domain and its subdomains (`example.org`).
    DomainAndSubdomains(String),
}

/// A URL pattern of a `web` permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlPattern {
    pub scheme: String,
    pub host: HostPattern,
    /// `None`: only the default port of the scheme.
    pub port: Option<u16>,
    pub path: String,
    /// `true`: the pattern ended in `*` and matches every path starting with [`Self::path`].
    pub path_prefix: bool,
}

/// What a request is about, already resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestTarget {
    /// For kinds without targets (`notifications`).
    Any,
    Table(ExtensionTable),
    Path(PathBuf),
    Url(WebRequest),
    MailServer {
        host: String,
        port: u16,
    },
    Program(PathBuf),
    Tag(String),
    StorageId(String),
}

/// A parsed http(s) request URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebRequest {
    pub scheme: String,
    /// Lower case.
    pub host: String,
    /// `None` for the default port of the scheme.
    pub port: Option<u16>,
    pub path: String,
}

impl WebRequest {
    /// Parses an `http` or `https` URL; anything else is not a web request.
    pub fn parse(value: &str) -> Option<Self> {
        let url = reqwest::Url::parse(value).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        Some(Self {
            scheme: url.scheme().to_owned(),
            host: url.host_str()?.to_ascii_lowercase(),
            port: url.port(),
            path: url.path().to_owned(),
        })
    }
}

impl Target {
    /// Parses the target of `kind`. A target the kind does not accept is absent (FR-022); a
    /// `database` target that is not an extension prefix or table is never valid.
    pub fn parse(kind: PermissionKind, value: &str) -> Option<Self> {
        use PermissionKind as K;
        match kind {
            K::Database => parse_database(value),
            K::Filesystem if value == "*" => Some(Self::Any),
            K::Filesystem => {
                let path = value.strip_suffix("/*").unwrap_or(value);
                clean_absolute(path).map(Self::Path)
            }
            K::Web if value == "*" => Some(Self::Any),
            K::Web if value.contains("://") => parse_url_pattern(value).map(Self::Url),
            K::Web => parse_domain(value).map(Self::Domain),
            K::Notifications => (value == "*").then_some(Self::Any),
            K::Passwords if value == "*" => Some(Self::Any),
            K::Passwords => {
                let tag = fold(value);
                (!tag.is_empty()).then_some(Self::Tag(tag))
            }
            K::RemoteStorage if value == "*" => Some(Self::Any),
            K::RemoteStorage => (!value.is_empty()).then(|| Self::StorageId(value.to_owned())),
            K::Mail => parse_mail_server(value),
            K::Shell if value == "*" => Some(Self::Any),
            K::Shell => clean_absolute(value).map(Self::Program),
        }
    }

    /// Whether `request` falls under this target. Targets and requests of different kinds never
    /// match.
    pub fn matches(&self, request: &RequestTarget) -> bool {
        match (self, request) {
            (Self::Any, RequestTarget::Table(_)) => false,
            (Self::Any, _) => true,
            (Self::ExtensionTables(prefix), RequestTarget::Table(table)) => &table.prefix == prefix,
            (Self::ExtensionTable(own), RequestTarget::Table(table)) => own == table,
            (Self::Path(root), RequestTarget::Path(path)) => path.starts_with(root),
            (Self::Url(pattern), RequestTarget::Url(url)) => pattern.matches(url),
            (Self::Domain(host), RequestTarget::Url(url)) => host.matches(&url.host),
            (
                Self::MailServer { host, port },
                RequestTarget::MailServer {
                    host: asked,
                    port: asked_port,
                },
            ) => host.eq_ignore_ascii_case(asked) && port.is_none_or(|port| port == *asked_port),
            (Self::Program(program), RequestTarget::Program(asked)) => program == asked,
            (Self::Tag(tag), RequestTarget::Tag(asked)) => *tag == fold(asked),
            (Self::StorageId(id), RequestTarget::StorageId(asked)) => id == asked,
            _ => false,
        }
    }
}

impl HostPattern {
    pub fn matches(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        let below = |domain: &str| {
            host.strip_suffix(domain)
                .is_some_and(|rest| rest.ends_with('.') && rest.len() > 1)
        };
        match self {
            Self::Exact(exact) => host == *exact,
            Self::Subdomains(domain) => below(domain),
            Self::DomainAndSubdomains(domain) => host == *domain || below(domain),
        }
    }
}

impl UrlPattern {
    pub fn matches(&self, url: &WebRequest) -> bool {
        self.scheme == url.scheme
            && self.host.matches(&url.host)
            && self.port == url.port
            && if self.path_prefix {
                url.path.starts_with(&self.path)
            } else {
                url.path == self.path
            }
    }
}

/// `<publicKey>__<name>__*` or one table of an extension.
fn parse_database(value: &str) -> Option<Target> {
    if let Some(base) = value.strip_suffix("__*") {
        let (key, name) = base.split_once("__")?;
        if name.contains("__") {
            return None;
        }
        let public_key = PublicKey::parse_case_insensitive(key).ok()?;
        let name = ExtensionName::parse(&name.to_ascii_lowercase()).ok()?;
        return Some(Target::ExtensionTables(TablePrefix { public_key, name }));
    }
    ExtensionTable::parse(value)
        .ok()
        .map(Target::ExtensionTable)
}

/// An absolute path without `.` or `..` components.
fn clean_absolute(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    let clean = path.is_absolute()
        && path
            .components()
            .all(|c| !matches!(c, Component::CurDir | Component::ParentDir));
    clean.then(|| path.to_path_buf())
}

/// A host name: lower-case letters, digits, `-` and `.`, no empty label.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn parse_host_pattern(value: &str, bare: bool) -> Option<HostPattern> {
    let value = value.to_ascii_lowercase();
    if let Some(domain) = value.strip_prefix("*.") {
        return valid_host(domain).then(|| HostPattern::Subdomains(domain.to_owned()));
    }
    if !valid_host(&value) {
        return None;
    }
    Some(if bare {
        HostPattern::DomainAndSubdomains(value)
    } else {
        HostPattern::Exact(value)
    })
}

fn parse_domain(value: &str) -> Option<HostPattern> {
    parse_host_pattern(value, true)
}

fn parse_url_pattern(value: &str) -> Option<UrlPattern> {
    let (scheme, rest) = value.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port.parse::<u16>().ok()?)),
        None => (authority, None),
    };
    let default_port = if scheme == "https" { 443 } else { 80 };
    let (path, path_prefix) = match path.strip_suffix('*') {
        Some(prefix) => (prefix, true),
        None => (path, false),
    };
    Some(UrlPattern {
        scheme: scheme.to_owned(),
        host: parse_host_pattern(host, false)?,
        port: port.filter(|port| *port != default_port),
        path: path.to_owned(),
        path_prefix,
    })
}

fn parse_mail_server(value: &str) -> Option<Target> {
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) => (host, Some(port.parse::<u16>().ok()?)),
        None => (value, None),
    };
    let host = host.to_ascii_lowercase();
    valid_host(&host).then_some(Target::MailServer { host, port })
}
