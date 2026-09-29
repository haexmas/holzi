//! The rendezvous meeting of a link (contracts/nostr-events.md, inner event
//! kind 24102): where the new installation says it can be reached, to the
//! key pair derived from the code. Same three layers as a presence meeting;
//! the seal is signed with the new installation's fresh device key.

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use iroh::{EndpointAddr, RelayUrl, TransportAddr};
use nostr::event::{Event, Kind};
use nostr::key::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};

use crate::sync::keys::DeviceKeys;
use crate::sync::presence::{hex_bytes32, unwrap_payload, wrap_payload, PresenceError};

/// The inner event kind of a link meeting.
const LINK_KIND: Kind = Kind::Custom(24102);
/// How stale a meeting's `ts` may be; the new installation renews it
/// while it waits.
const MAX_AGE_MS: u64 = 150_000;
/// How far into the future a meeting's `ts` may claim to be.
const MAX_FUTURE_MS: u64 = 30_000;

/// Where the new installation can be reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkMeeting {
    pub v: u8,
    #[serde(with = "hex_bytes32")]
    pub endpoint: [u8; 32],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iroh_relay: Option<String>,
    pub addrs: Vec<String>,
    pub ts: u64,
}

impl LinkMeeting {
    /// The meeting for an endpoint reachable at `addr`, stamped now.
    pub fn new(addr: &EndpointAddr) -> Self {
        Self {
            v: 1,
            endpoint: *addr.id.as_bytes(),
            iroh_relay: addr.relay_urls().next().map(RelayUrl::to_string),
            addrs: addr.ip_addrs().map(SocketAddr::to_string).collect(),
            ts: now_ms(),
        }
    }

    /// Whether `ts` is recent enough to act on.
    pub fn is_fresh(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.ts) <= MAX_AGE_MS
            && self.ts.saturating_sub(now_ms) <= MAX_FUTURE_MS
    }

    /// The address to dial.
    pub fn endpoint_addr(&self) -> Result<EndpointAddr, PresenceError> {
        let id =
            iroh::EndpointId::from_bytes(&self.endpoint).map_err(|_| PresenceError::InvalidKey)?;
        let mut addrs: Vec<TransportAddr> = self
            .addrs
            .iter()
            .filter_map(|a| a.parse::<SocketAddr>().ok())
            .map(TransportAddr::Ip)
            .collect();
        if let Some(url) = self
            .iroh_relay
            .as_deref()
            .and_then(|r| r.parse::<RelayUrl>().ok())
        {
            addrs.push(TransportAddr::Relay(url));
        }
        Ok(EndpointAddr::from_parts(id, addrs))
    }
}

/// Builds the gift-wrapped meeting for the rendezvous key `rv_pk`.
pub fn build(
    sender: &DeviceKeys,
    meeting: &LinkMeeting,
    rv_pk: &PublicKey,
) -> Result<Event, PresenceError> {
    let json =
        serde_json::to_string(meeting).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    wrap_payload(LINK_KIND, &sender.device_secret, &json, rv_pk)
}

/// Opens a meeting sent to the rendezvous key `rv_sk`: the device key that
/// sealed it and where it can be reached.
pub fn open(event: &Event, rv_sk: &SecretKey) -> Result<([u8; 32], LinkMeeting), PresenceError> {
    let (sender, json) = unwrap_payload(event, rv_sk, LINK_KIND)?;
    let meeting =
        serde_json::from_str(&json).map_err(|e| PresenceError::Malformed(e.to_string()))?;
    Ok((sender, meeting))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "meeting_tests.rs"]
mod tests;
