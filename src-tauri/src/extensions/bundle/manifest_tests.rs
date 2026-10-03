use std::path::Path;

use super::*;
use crate::extensions::bundle::verify_bundle;
use crate::extensions::permissions::PermissionKind;

fn vector(name: &str) -> VerifiedBundle {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extension_bundles")
        .join(name);
    verify_bundle(&std::fs::read(path).expect("vector")).expect("valid vector")
}

#[test]
fn the_notes_like_vector_gives_every_field() {
    let manifest = Manifest::from_verified(&vector("good-notes-like.xt")).unwrap();
    assert_eq!(manifest.name.as_str(), "notes-like");
    assert_eq!(manifest.version, semver::Version::new(1, 2, 0));
    assert_eq!(
        manifest.public_key.as_str(),
        "3614253f84ba66a8faa168d317a6979992979a3e7ad15ae83ca2823f5ca97d34"
    );
    assert_eq!(manifest.title(), "Notes");
    assert_eq!(manifest.author.as_deref(), Some("Test Author"));
    assert_eq!(manifest.homepage, None, "null counts as absent");
    assert_eq!(manifest.icon.as_deref(), Some("icon.svg"));
    assert_eq!(manifest.entry, "index.html");
    assert!(!manifest.single_instance);
    assert_eq!(
        manifest.migrations_dir.as_deref(),
        Some("database/migrations")
    );
    assert_eq!(
        manifest.i18n.as_ref().unwrap()["en"]["name"],
        serde_json::json!("Notes")
    );
    assert!(manifest.permissions.declared.is_empty());
}

#[test]
fn a_minimal_manifest_falls_back_to_name_and_index_html() {
    let manifest = Manifest::from_verified(&vector("good-minimal.xt")).unwrap();
    assert_eq!(manifest.title(), manifest.name.as_str());
    assert_eq!(manifest.entry, "index.html");
    assert_eq!(manifest.migrations_dir, None);
}

#[test]
fn a_prerelease_version_is_kept() {
    let manifest = Manifest::from_verified(&vector("good-semver-prerelease.xt")).unwrap();
    assert!(!manifest.version.pre.is_empty());
}

#[test]
fn declared_permissions_are_mapped() {
    let mut bundle = vector("good-minimal.xt");
    bundle.manifest.insert(
        "permissions".into(),
        haex_bundle::jcs::parse_restricted(
            r#"{"http":[{"target":"https://example.org/*"}],"spaces":[{"target":"*"}]}"#,
        )
        .unwrap(),
    );
    let manifest = Manifest::from_verified(&bundle).unwrap();
    assert_eq!(manifest.permissions.declared.len(), 1);
    assert_eq!(manifest.permissions.declared[0].kind, PermissionKind::Web);
    assert_eq!(manifest.permissions.unsupported_categories, ["spaces"]);
}

#[test]
fn the_stored_manifest_bytes_give_the_same_view() {
    let bundle = vector("good-notes-like.xt");
    assert_eq!(
        Manifest::from_stored(&bundle.manifest_bytes).unwrap(),
        Manifest::from_verified(&bundle).unwrap()
    );
    assert!(Manifest::from_stored(b"{ }").is_err());
}
