//! Preferred Nostr- and iroh-Relays for own-device sync (spec 024, FR-008,
//! research R7).
//!
//! The built-in servers are always listed and can be switched off but not
//! removed; the servers a user adds are listed after them and can be switched
//! off or removed. What is used is everything listed that is not switched off.
//! Read from the vault-wide `preferences` table; unset or unparseable reads as
//! "nothing added, nothing switched off". A relay that cannot be reached
//! never blocks local work (Constitution VII): callers only log a
//! failure to apply a relay change, they never fail the vault open or the
//! sync service over it.

use std::sync::Arc;

use haex_crdt::CrdtTransaction;
use iroh::{RelayConfig, RelayMap, RelayMode, RelayUrl};
use nostr::types::RelayUrl as NostrRelayUrl;

use crate::storage::preferences::{self, PrefScope};
use crate::storage::query::Query;

/// Preference key for the Nostr relays this device publishes presence to
/// and subscribes on, besides the built-in ones. Value: a JSON array of
/// relay URLs.
pub const PREF_NOSTR_RELAYS: &str = "sync.servers.nostr";
/// Preference key for the iroh-Relays used to reach other devices, besides
/// iroh's own production relays. Value: a JSON array of relay URLs.
pub const PREF_IROH_RELAYS: &str = "sync.servers.iroh";
/// Preference key for the servers switched off, built-in or added, of either
/// kind. Value: a JSON array of relay URLs.
pub const PREF_DISABLED_RELAYS: &str = "sync.servers.disabled";

/// The Nostr relays used when nothing is configured (research R7):
/// public relays checked to support ephemeral events (NIP-11).
pub fn default_nostr_relays() -> Vec<String> {
    vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.primal.net".to_string(),
    ]
}

/// The iroh-Relays used when nothing is configured: the ones iroh itself
/// ships ([`RelayMode::Default`]), written the way a person would type them
/// (no trailing `/` or root-label `.`) and sorted so the settings list them
/// in a stable order.
pub fn default_iroh_relays() -> Vec<String> {
    let mut urls: Vec<String> = RelayMode::Default
        .relay_map()
        .urls::<Vec<RelayUrl>>()
        .iter()
        .map(pretty)
        .collect();
    urls.sort();
    urls
}

/// A relay URL the way a person would type it.
fn pretty(url: &RelayUrl) -> String {
    let text = url.to_string();
    text.trim_end_matches('/').trim_end_matches('.').to_string()
}

/// The server preferences as currently stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerConfig {
    /// The Nostr servers added besides the built-in ones.
    pub nostr_relays: Vec<String>,
    /// The iroh servers added besides the built-in ones.
    pub iroh_relays: Vec<String>,
    /// The servers, of either kind, that are switched off.
    pub disabled: Vec<String>,
}

impl ServerConfig {
    /// Only the given servers: the built-in ones are switched off. For tests
    /// and tooling that must not reach a public server.
    pub fn only(nostr: Vec<String>, iroh: Vec<String>) -> Self {
        let disabled = default_nostr_relays()
            .into_iter()
            .chain(default_iroh_relays())
            .collect();
        Self {
            nostr_relays: nostr,
            iroh_relays: iroh,
            disabled,
        }
    }

    /// The Nostr relays to actually use: built-in and added, not switched off.
    pub fn effective_nostr_relays(&self) -> Vec<String> {
        self.in_use(default_nostr_relays(), &self.nostr_relays)
    }

    /// The iroh-Relays to actually use: built-in and added, not switched off.
    pub fn effective_iroh_relays(&self) -> Vec<String> {
        self.in_use(default_iroh_relays(), &self.iroh_relays)
    }

    fn in_use(&self, defaults: Vec<String>, added: &[String]) -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        for url in defaults.iter().chain(added) {
            if !self.disabled.contains(url) && !urls.contains(url) {
                urls.push(url.clone());
            }
        }
        urls
    }

    /// The iroh `RelayMode` to bind or reconfigure the endpoint with. None
    /// in use means no relay: devices then reach each other directly only.
    pub fn relay_mode(&self) -> RelayMode {
        let in_use = self.effective_iroh_relays();
        if in_use == default_iroh_relays() {
            return RelayMode::Default;
        }
        if in_use.is_empty() {
            return RelayMode::Disabled;
        }
        // The built-in ones keep their own configuration (their host names
        // end in a root-label dot the listed form leaves out).
        let mut configs: Vec<RelayConfig> = RelayMode::Default
            .relay_map()
            .relays::<Vec<Arc<RelayConfig>>>()
            .into_iter()
            .filter(|config| in_use.contains(&pretty(&config.url)))
            .map(|config| (*config).clone())
            .collect();
        for url in in_use
            .iter()
            .filter(|url| !default_iroh_relays().contains(url))
        {
            match url.parse::<RelayUrl>() {
                Ok(url) => configs.push(RelayConfig::from(url)),
                Err(error) => {
                    log::warn!(
                        "sync: {PREF_IROH_RELAYS} has an invalid relay URL {url:?}: {error}"
                    );
                }
            }
        }
        RelayMode::Custom(RelayMap::from_iter(configs))
    }
}

/// Reads the current server preferences; a missing or unparseable value
/// reads as empty, never as an error.
pub fn read(q: &mut impl Query) -> haex_crdt::Result<ServerConfig> {
    Ok(ServerConfig {
        nostr_relays: read_urls(q, PREF_NOSTR_RELAYS)?,
        iroh_relays: read_urls(q, PREF_IROH_RELAYS)?,
        disabled: read_urls(q, PREF_DISABLED_RELAYS)?,
    })
}

/// Reads one URL-list preference; unset or undecodable reads as empty.
fn read_urls(q: &mut impl Query, key: &str) -> haex_crdt::Result<Vec<String>> {
    let Some(raw) = preferences::get(q, PrefScope::Vault, key)? else {
        return Ok(Vec::new());
    };
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(urls) => Ok(urls),
        Err(error) => {
            log::warn!("sync: {key} does not decode as a URL list, reading it as empty: {error}");
            Ok(Vec::new())
        }
    }
}

/// Most servers of one kind the settings accept.
const MAX_SERVERS: usize = 10;

/// Checks the servers a user added: Nostr servers are `ws://` or `wss://`
/// URLs, iroh servers `http://` or `https://`. The error names the first
/// that is not, and is the user's to fix, not a failure.
pub fn validate(config: &ServerConfig) -> Result<(), String> {
    check_list(
        &config.nostr_relays,
        &["ws", "wss"],
        "a Nostr server",
        |url| NostrRelayUrl::parse(url).is_ok(),
    )?;
    check_list(
        &config.iroh_relays,
        &["http", "https"],
        "an iroh server",
        |url| url.parse::<RelayUrl>().is_ok(),
    )?;
    // Switched off servers are only compared with, never dialed.
    if config.disabled.len() > 4 * MAX_SERVERS || config.disabled.iter().any(|url| url.len() > 256)
    {
        return Err("too many servers switched off".to_string());
    }
    Ok(())
}

fn check_list(
    urls: &[String],
    schemes: &[&str],
    what: &str,
    parses: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if urls.len() > MAX_SERVERS {
        return Err(format!("at most {MAX_SERVERS} servers of one kind"));
    }
    for url in urls {
        let scheme_ok = url
            .split_once("://")
            .is_some_and(|(scheme, rest)| schemes.contains(&scheme) && !rest.is_empty());
        if !scheme_ok || !parses(url) || url.len() > 256 || url.chars().any(char::is_whitespace) {
            return Err(format!("{url:?} is not {what}"));
        }
    }
    Ok(())
}

/// Stores the servers, replacing the stored ones.
pub fn write(tx: &mut CrdtTransaction<'_>, config: &ServerConfig) -> haex_crdt::Result<()> {
    for (key, urls) in [
        (PREF_NOSTR_RELAYS, &config.nostr_relays),
        (PREF_IROH_RELAYS, &config.iroh_relays),
        (PREF_DISABLED_RELAYS, &config.disabled),
    ] {
        let json =
            serde_json::to_string(urls).map_err(|e| haex_crdt::Error::consumer(e.to_string()))?;
        preferences::insert_or_update(tx, PrefScope::Vault, key, &json)?;
    }
    Ok(())
}

/// Parses `urls`, logging and dropping anything that does not parse as a
/// relay URL instead of failing the whole list.
pub fn parse_relay_urls(urls: &[String]) -> Vec<RelayUrl> {
    urls.iter()
        .filter_map(|url| match url.parse() {
            Ok(url) => Some(url),
            Err(error) => {
                log::warn!("sync: {PREF_IROH_RELAYS} has an invalid relay URL {url:?}: {error}");
                None
            }
        })
        .collect()
}

/// What to add and remove on a running endpoint to move from `have` to
/// `want` (FR-008: relay changes apply at runtime, not just at the next
/// bind). `Endpoint` has no getter for its current relay set, so the
/// caller tracks `have` itself (see `SyncNode::apply_relays`).
pub fn diff_relays(have: &[RelayUrl], want: &[RelayUrl]) -> (Vec<RelayUrl>, Vec<RelayUrl>) {
    let to_insert = want.iter().filter(|u| !have.contains(u)).cloned().collect();
    let to_remove = have.iter().filter(|u| !want.contains(u)).cloned().collect();
    (to_insert, to_remove)
}

#[cfg(test)]
#[path = "servers_tests.rs"]
mod tests;
