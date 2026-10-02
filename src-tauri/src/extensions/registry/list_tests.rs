use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::VaultGate;

fn vector(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles")
            .join(name),
    )
    .unwrap()
}

#[test]
fn installed_extensions_are_listed_with_title_version_and_icon() {
    let (_dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db.clone())).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let notes = install(
        &vault,
        &vector("good-notes-like.xt"),
        vec![],
        false,
        device,
        1,
    )
    .unwrap()
    .ids
    .extension_id;
    install(&vault, &vector("good-minimal.xt"), vec![], false, device, 1).unwrap();

    let list = vault
        .read_blocking(move |q| list(q, device).map_err(Into::into))
        .unwrap();
    assert_eq!(list.len(), 2);
    let entry = list.iter().find(|e| e.id == notes.to_string()).unwrap();
    assert_eq!(entry.title, "Notes");
    assert_eq!(entry.version.as_deref(), Some("1.2.0"));
    assert!(entry.enabled && entry.has_icon && !entry.single_instance);
    assert_eq!(entry.state, "installed");
    assert_eq!(entry.status_here, None);

    let icon = vault
        .read_blocking(move |q| icon_data_url(q, notes).map_err(Into::into))
        .unwrap()
        .unwrap();
    assert!(icon.starts_with("data:image/svg+xml;base64,"), "{icon}");
}
