//! The extension bundles of the e2e scenes of spec 017 (`scripts/e2e/scenarios/extension-*.test.ts`),
//! built with the one implementation of the bundle format (`haex-bundle`) and a test key derived
//! from a public seed: it signs these fixtures only and must never be trusted elsewhere.
//!
//! The committed `.xt` files must equal what this builds; `HOLZI_WRITE_E2E_FIXTURES=1` rewrites
//! them after a change.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use haex_bundle::jcs::parse_restricted;
use haex_bundle::{build_archive, Entry};
use sha2::{Digest, Sha256};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extension_e2e")
}

fn test_key() -> SigningKey {
    let seed =
        Sha256::digest(b"holzi spec 017 e2e fixtures only, never use for real signing: probe");
    SigningKey::from_bytes(&seed.into())
}

fn probe() -> Vec<u8> {
    let key = test_key();
    let public_key: String = key
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let table = format!("{public_key}__probe__items");
    let manifest = parse_restricted(
        r#"{"name":"probe","version":"1.0.0","displayName":"Probe","migrationsDir":"db",
            "permissions":{"filesystem":[{"target":"/tmp/holzi-probe","operation":"read"}]}}"#,
    )
    .expect("manifest");
    let page = std::fs::read(dir().join("probe.html")).expect("probe page");
    let module = std::fs::read(dir().join("probe.js")).expect("probe module");
    let files = vec![
        Entry {
            path: "index.html".into(),
            data: page,
        },
        Entry {
            path: "probe.js".into(),
            data: module,
        },
        Entry {
            path: "db/0000_init.sql".into(),
            data: format!("CREATE TABLE `{table}` (`id` text PRIMARY KEY NOT NULL, `label` text);")
                .into_bytes(),
        },
    ];
    build_archive(files, manifest, &key).expect("probe bundle")
}

#[test]
fn the_committed_e2e_bundles_are_what_this_builds() {
    let built = probe();
    let path = dir().join("probe.xt");
    if std::env::var_os("HOLZI_WRITE_E2E_FIXTURES").is_some() {
        std::fs::write(&path, &built).expect("write probe.xt");
    }
    let committed = std::fs::read(&path).expect("probe.xt (HOLZI_WRITE_E2E_FIXTURES=1 builds it)");
    assert!(
        committed == built,
        "probe.xt is stale; rebuild with HOLZI_WRITE_E2E_FIXTURES=1"
    );
}
