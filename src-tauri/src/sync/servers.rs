//! Preferred Nostr- and iroh-Relays for own-device sync (spec 024, FR-008,
//! research R7).
//!
//! Read from the vault-wide `preferences` table; unset or unparseable
//! falls back to the built-in defaults. A relay that cannot be reached
//! never blocks local work (Constitution VII): callers only log a
//! failure to apply a relay change, they never fail the vault open or the
//! sync service over it.

use haex_crdt::CrdtTransaction;
use iroh::{RelayMap, RelayMode, RelayUrl};

use crate::storage::preferences::{self, PrefScope};
use crate::storage::query::Query;

/// Preference key for the Nostr relays this device publishes presence to
/// and subscribes on. Value: a JSON array of relay URLs.
pub const PREF_NOSTR_RELAYS: &str = "sync.servers.nostr";
/// Preference key for the iroh-Relays used to reach other devices. Value:
/// a JSON array of relay URLs. An empty or absent value means "use iroh's
/// own production relays" ([`RelayMode::Default`]), not "use none".
pub const PREF_IROH_RELAYS: &str = "sync.servers.iroh";

/// The Nostr relays used when nothing is configured (research R7):
/// public relays checked to support ephemeral events (NIP-11).
pub fn default_nostr_relays() -> Vec<String> {
    vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.primal.net".to_string(),
    ]
}

/// The server preferences as currently stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerConfig {
    /// Empty means "use [`default_nostr_relays`]".
    pub nostr_relays: Vec<String>,
    /// Empty means "use iroh's own default relays" ([`RelayMode::Default`]).
    pub iroh_relays: Vec<String>,
}

impl ServerConfig {
    /// The Nostr relays to actually use: configured, or the defaults.
    pub fn effective_nostr_relays(&self) -> Vec<String> {
        if self.nostr_relays.is_empty() {
            default_nostr_relays()
        } else {
            self.nostr_relays.clone()
        }
    }

    /// The iroh `RelayMode` to bind or reconfigure the endpoint with.
    pub fn relay_mode(&self) -> RelayMode {
        if self.iroh_relays.is_empty() {
            return RelayMode::Default;
        }
        match RelayMap::try_from_iter(self.iroh_relays.iter().map(String::as_str)) {
            Ok(map) => RelayMode::Custom(map),
            Err(error) => {
                log::warn!(
                    "sync: {PREF_IROH_RELAYS} has an invalid relay URL, using defaults: {error}"
                );
                RelayMode::Default
            }
        }
    }
}

/// Reads the current server preferences; a missing or unparseable value
/// reads as empty (falls back to defaults), never as an error.
pub fn read(q: &mut impl Query) -> haex_crdt::Result<ServerConfig> {
    Ok(ServerConfig {
        nostr_relays: read_urls(q, PREF_NOSTR_RELAYS)?,
        iroh_relays: read_urls(q, PREF_IROH_RELAYS)?,
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
            log::warn!("sync: {key} does not decode as a URL list, using defaults: {error}");
            Ok(Vec::new())
        }
    }
}

/// Most servers of one kind the settings accept.
const MAX_SERVERS: usize = 10;

/// Checks the servers a user typed: Nostr servers are `ws://` or `wss://`
/// URLs, iroh servers `http://` or `https://`. The error names the first
/// that is not, and is the user's to fix, not a failure.
pub fn validate(nostr: &[String], iroh: &[String]) -> Result<(), String> {
    check_list(nostr, &["ws", "wss"], "a Nostr server")?;
    check_list(iroh, &["http", "https"], "an iroh server")
}

fn check_list(urls: &[String], schemes: &[&str], what: &str) -> Result<(), String> {
    if urls.len() > MAX_SERVERS {
        return Err(format!("at most {MAX_SERVERS} servers of one kind"));
    }
    for url in urls {
        let scheme_ok = url
            .split_once("://")
            .is_some_and(|(scheme, rest)| schemes.contains(&scheme) && !rest.is_empty());
        if !scheme_ok || url.len() > 256 || url.chars().any(char::is_whitespace) {
            return Err(format!("{url:?} is not {what}"));
        }
    }
    Ok(())
}

/// Stores the servers, replacing the stored ones; an empty list brings back
/// the defaults.
pub fn write(
    tx: &mut CrdtTransaction<'_>,
    nostr: &[String],
    iroh: &[String],
) -> haex_crdt::Result<()> {
    for (key, urls) in [(PREF_NOSTR_RELAYS, nostr), (PREF_IROH_RELAYS, iroh)] {
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
