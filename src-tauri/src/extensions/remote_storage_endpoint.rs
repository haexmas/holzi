//! An endpoint an extension proposes for a new storage (spec 038 FR-009b, FR-017, research R8),
//! checked before holzi shows its dialog: its form, the permission `remoteStorage`/`add` for its
//! host, and that all its addresses lie in one scope holzi reaches (`http` only to a local one).
//! The scope found here is what the dialog shows and what the new connection must keep.

use std::time::Duration;

use reqwest::Url;
use url::Host;

use super::bridge::blocking::block_on;
use super::bridge::dispatch::CallContext;
use super::error::BridgeError;
use super::permissions::{Action, RequestTarget};
use super::remote_storage::{check, grants, invalid, provider};
use crate::remote_storage::address::{self, AddressError};
use crate::remote_storage::model::ConnectionView;
use crate::remote_storage::EndpointScope;

/// How long holzi waits for the addresses of a proposed endpoint before the dialog.
const RESOLVE_TIME: Duration = Duration::from_secs(10);

/// A proposed endpoint after its checks.
pub(super) struct Proposed {
    pub url: Url,
    pub scope: EndpointScope,
}

impl Proposed {
    /// Checks `endpoint` (see the module): 3001 for a form or an address holzi never reaches,
    /// 1004/1002 without the `add` permission for its host, 2002 `network` when it does not
    /// resolve.
    pub fn check(ctx: &CallContext, endpoint: &str) -> Result<Self, BridgeError> {
        let url = address::check_endpoint(endpoint).map_err(|_| invalid("endpoint not allowed"))?;
        let host = match url.host() {
            Some(Host::Domain(name)) => name.to_ascii_lowercase(),
            Some(Host::Ipv4(ip)) => ip.to_string(),
            Some(Host::Ipv6(ip)) => format!("[{ip}]"),
            None => return Err(invalid("endpoint not allowed")),
        };
        let port = url
            .port_or_known_default()
            .ok_or_else(|| invalid("endpoint not allowed"))?;
        check(
            ctx,
            &grants(ctx)?,
            Action::Add,
            RequestTarget::Endpoint {
                host: host.clone(),
                port,
            },
            &format!("{host}:{port}"),
        )?;
        let resolver = ctx.host.storage.resolver();
        let scope = block_on(async {
            tokio::time::timeout(RESOLVE_TIME, address::scope_of(&url, resolver.as_ref())).await
        });
        match scope {
            Ok(Ok(scope)) => Ok(Self { url, scope }),
            Ok(Err(AddressError::Invalid | AddressError::NotAllowed)) => {
                Err(invalid("endpoint not allowed"))
            }
            Ok(Err(AddressError::Unresolved)) | Err(_) => Err(provider("network")),
        }
    }

    /// Whether the user may pick `connection` instead of new credentials: it talks to this
    /// endpoint in `region`, in the scope the dialog shows.
    pub fn fits(&self, connection: &ConnectionView, region: &str) -> bool {
        Url::parse(&connection.endpoint).is_ok_and(|own| own == self.url)
            && connection.region == region
            && connection.endpoint_scope == self.scope
    }
}
