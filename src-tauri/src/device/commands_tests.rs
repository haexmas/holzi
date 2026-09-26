//! Tests for the ordering of `list_vault_devices` (spec 023-settings-app, FR-022, contracts §5a).

use uuid::Uuid;

use super::{order_vault_devices, VaultDevicePayload};
use crate::storage::known_devices::KnownDevice;

fn device(alias: Option<&str>) -> KnownDevice {
    KnownDevice {
        installation_uuid: Uuid::new_v4(),
        vault_device_uuid: Uuid::new_v4(),
        alias: alias.map(str::to_owned),
    }
}

fn names(devices: &[VaultDevicePayload]) -> Vec<Option<&str>> {
    devices.iter().map(|d| d.alias.as_deref()).collect()
}

#[test]
fn puts_this_device_first_then_by_name_ignoring_case_and_unnamed_last() {
    let this = device(Some("Zeta"));
    let current = this.installation_uuid;
    let devices = vec![
        device(None),
        device(Some("Beta")),
        this,
        device(Some("alpha")),
        device(None),
    ];

    let ordered = order_vault_devices(devices, current);

    assert_eq!(
        names(&ordered),
        vec![Some("Zeta"), Some("alpha"), Some("Beta"), None, None]
    );
    assert_eq!(ordered.iter().filter(|d| d.is_current).count(), 1);
    assert!(ordered[0].is_current);
}

#[test]
fn marks_nothing_when_this_installation_is_missing() {
    let ordered = order_vault_devices(vec![device(Some("A"))], Uuid::new_v4());

    assert_eq!(ordered.len(), 1);
    assert!(!ordered[0].is_current);
}

#[test]
fn serializes_camel_case_with_a_null_name() {
    let payload = VaultDevicePayload {
        vault_device_uuid: Uuid::nil(),
        alias: None,
        is_current: true,
    };

    let json = serde_json::to_value(&payload).expect("serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "vaultDeviceUuid": "00000000-0000-0000-0000-000000000000",
            "alias": null,
            "isCurrent": true,
        })
    );
}
