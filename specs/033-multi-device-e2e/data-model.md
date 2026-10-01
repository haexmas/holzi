# Data Model: End-to-End Tests Across Several Vaults and Devices

The feature stores nothing in the application. These are the objects of the test code; fields are what a scenario can see.

## Group

One per scenario that uses several vaults. Created by the group helper, ended with the scenario.

| Field     | Meaning                                                      |
| --------- | ------------------------------------------------------------ |
| `name`    | scenario name; prefix of all device roots                    |
| `relay`   | the group's test relay (see Test relay)                      |
| `users`   | map of user name to User                                     |
| `devices` | flat map of device name to Device, names unique in the group |

Rules: device names are unique across users; a group with more devices than `E2E_MAX_DEVICES` is refused before any device starts (FR-019); two groups never share a root, port or relay (FR-018).

## User

A person with one vault. Fields: `name`, `vaultName`, `passphrase` (random per run, never a literal), `devices` (ordered, the first is the first device that created the vault). Different users have different vaults and never share data unless the feature under test shares it.

## Device

One running application process of a vault, addressed by `<user>/<device>`. Fields a scenario can read:

| Field          | Meaning                                                               |
| -------------- | --------------------------------------------------------------------- |
| `name`, `user` | address                                                               |
| `host`         | the `DeviceHost` handle (driver layer)                                |
| `role`         | `main` or `linked`, as set when the device was created or linked      |
| `state`        | `running`, `stopped`, `offline` (running, network disabled), `killed` |
| `page`         | the `Page` operations of spec 016 for this device                     |

State transitions: `stopped -> running` (start), `running -> stopped` (stop), `running -> killed` (kill), `running -> offline` (goOffline: network disabled while running), `offline -> running` (goOnline: network restored while running), `killed -> running` (start over the same data). Any state ends at `stopped` when the scenario ends.

## Test relay

A Nostr relay process of the group. Fields: `url` (fixed for the group's life), `state` (`up` or `down`). Transitions: `up -> down` (stop), `down -> up` (start on the same port). Messages are lost on restart.

## DeviceHost (driver layer)

The platform-independent interface; see [contracts/driver-layer.md](contracts/driver-layer.md). One implementation, for Linux.

## Failure material

Per scenario and per device: a log, a screenshot while the device lives, and, on failure or `--keep`, the device's data folder; plus the scenario's timeline with a device on each step. Layout in [contracts/failure-material.md](contracts/failure-material.md).

## Platform document

`scripts/e2e/PLATFORMS.md`: one entry per further platform with driver, runner or device type, how several devices connect, known limits (research R9). Not code.
