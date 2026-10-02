# Research: End-to-End Tests Across Several Vaults and Devices

Facts come from the code of `main` at `4ea5292` (spec 024 merged, rig of spec 016). Decisions marked **Gate** need a measurement before the dependent stage is trusted.

## R1 What "online" and "zuletzt online" depend on (decides FR-014)

**Finding.** `online` is `is_current || connected.contains(pubkey)`: a device is online while it has a live session (`src-tauri/src/sync/device_view.rs:131`, peers in `endpoint.rs`). It has no time threshold. `lastSeen` is a stored wall-clock time, raised at session start and end, by fresh presence meetings and by peers' progress reports (`endpoint_devices.rs`, `presence.rs`, `seen.rs`). The text "gerade eben online" / "zuletzt online vor N Min." is computed in the interface from `lastSeen` and a 30 s ticker (`src/lib/sync/deviceStatus.ts`, `FederationView.vue`). The code has no clock abstraction; tokio's paused time cannot reach a spawned application.

**Decision.** No clock control in the application. The 60 s of M4 is a promise about how fast a vanished peer is noticed, not a timer that a scenario could skip. The scenario stops B and waits, with a fixed deadline of 60 s that `E2E_TIME_SCALE` does not stretch, until A's row for B shows `data-online` absent and a status text of the "zuletzt online" family; it then checks that the shown elapsed time is "gerade eben" or at most "vor 1 Min.". A graceful stop is noticed at once (the session closes with an explicit code, `session.rs:115`); the 60 s bound only matters for a process that dies without closing. The scenario covers both: a normal stop and a killed process (SIGKILL through the driver layer's `kill` operation).

**Alternatives.** An injectable clock in the sync code (rejected: touches production code in about eight places for a value that is not a timer); faking the system time of the instance (rejected: needs privileges and also breaks TLS and signatures). FR-014 is aligned in the spec accordingly (see Spec alignment in the plan).

**Measured (2026-10-02, `sync-presence`).** A stopped phone was shown as not online by the laptop after 34.6 s and a killed one after 30.0 s, so both are noticed at about the idle time of the connection, with a margin of nearly two over the 60 s deadline. A device stopped through the driver is not closed gracefully by the application (the driver ends the process group), so "stop" and "kill" look the same to the peer; a close that the application does itself (the lock, scenario M8) closes its sessions at once.

## R2 Taking a running device offline (decides FR-013)

**Finding.** Sessions between devices of one machine are direct QUIC connections. Their addresses reach other devices only through Nostr presence and are kept in memory (`MemoryLookup`, `endpoint.rs`), never on disk. `sync_servers_set` applies iroh relays at once but changes the Nostr relays of the presence loop only at the next opening of the vault (`commands.rs:469-485`, `service.rs`). There is no in-app offline switch.

**Decision (G4, measured 2026-10-02).** On Linux a running device is taken offline by restarting it with its server lists set to none: `goOffline(device)` sets the lists and restarts the application over the same data, `goOnline(device)` sets them back and restarts again. After such a restart the device holds no address of any peer (they live in memory only), so it neither reaches nor is reached by peers or the test relay, yet runs and works locally, which is what M7 and M9 need. The observable promise of FR-013 is kept; what is relaxed is the literal "without restarting the application", which the spec is amended to say.

**Why not in place.** Measured on this machine: inside a nested user and network namespace a veth link between two namespaces can be set down and up while a receiving process keeps running; traffic stops and resumes (a packet sent while down is delivered after the restore, because it waits for neighbour resolution). So an in-place cut is possible here. It is not adopted because: (a) every device, its driver, its virtual screen and the relay would have to start inside one nested user namespace with a network namespace per device and veth links, which changes `instance.ts`, the port handling and the run wrapper, as the driver's loopback port would no longer be reachable from the runner; (b) it needs unprivileged user namespaces, which are enabled here but are restricted by default on Ubuntu 24.04 runners (an AppArmor setting that a job can only lift with administrator rights; documented default, not measured on a runner in this PR, because that needs a push); (c) the observable behaviour for M7 and M9 is the same. A firewall rule per process needs root and was not tried. The rule the plan set for G4 was "no in-place mechanism without privileges on the runner means restart", and that is the case.

**Alternatives.** SIGSTOP of the process (rejected: a frozen app cannot do the local work M7 requires); a test-only command in the application (rejected: test code in a production path, and the build would need a feature the ordinary run does not use); network namespaces (see above; revisit if the project gets a runner where they are allowed, as a follow-up that replaces only the Linux implementation of `goOffline`/`goOnline`).

## R3 The test relay going down and up (FR-013)

**Finding.** `e2e_nostr_relay` is 19 lines around `MockRelay::run()` and picks a random port; a second start gets another port, so an outage cannot return on the same URL (`src-tauri/src/bin/e2e_nostr_relay.rs`, `lib/nostr-relay.ts`).

**Decision.** The library offers `LocalRelay::builder().port(p)` (shown in its own example `local-relay-simple`), so the binary can take an optional port (argument or one environment variable); the harness picks a free port itself and passes it, so `relay.stop()` followed by `relay.start()` returns on the same URL. State is lost on restart, which is fine: presence meetings are ephemeral. **Gate (G1):** the client library of the application must reconnect to a relay that returns, and within a measured fixed deadline; the servers scenario depends on it only through the settings, but the edge case "relay restarted while devices run" does. A failed reconnect remains documented as an application finding, but also fails the scenario, keeps CI red, and blocks the dependent Stage 2 work until the plan is amended.

**Gate G1 (2026-10-02, `sync-relay-return`).** Passed. With the relay stopped, both devices kept running and kept local work; a phone that restarted during the outage found the laptop again 2.8 s after the relay returned on the same URL, and both devices exchanged the work done in the pause. The laptop still holds the session of a vanished peer for about 30 s, so the scenario first waits until the laptop shows the phone as not online; otherwise "online" right after the relay returns would be the old session and prove nothing.

## R4 Linking through the start-page form (M1)

**Finding.** The form (`LinkSheet.vue`) has hooks for code, device name, vault name and its server list, but its submit button has none, and the passphrase fields have only ids (`#link-passphrase`, `#link-passphrase-confirm`, which the hook syntax accepts as selectors). It can set Nostr relays but not iroh relays; the existing helpers avoid the problem by calling `link_join_start` with servers, which the form cannot do.

**Decision.** Add one hook, `link-submit`, to the submit button (the only application change besides the relay binary). **Gate (G2):** whether a device joined through the form, with default iroh relays, connects to the host on loopback without waiting for or reaching the internet (spec 016 FR-007 keeps scenarios off the network). A measurement in Stage 2 decides. If it does, nothing else is needed. If it does not, the fallback is a build-time override of the default iroh relay list that exists only in debug builds and is read from one environment variable; it is documented in the plan's Complexity Tracking at that time and requires a reviewed amendment of this plan.

**Gate G2 (2026-10-02, `sync-link`).** Passed: a device joined through the start-page form, with the group relay as its only Nostr server and the default iroh relays, linked and opened the vault with all of its data on loopback; the whole scenario takes 29 s. Not measured: whether that device contacted the public iroh relays on a machine that has internet. If scenarios must stay off the network, the debug-build override described above remains the fallback; nothing needs it for the scenario to pass.

## R5 Failure material for several devices

**Finding.** `artifacts.ts` writes one screenshot (of the first instance only) and one shared `driver.log` per scenario, so lines of several devices interleave; instance roots are removed.

**Decision.** Device names label everything: `<runDir>/<scenario>/<encoded-user-length>-<encoded-user>-<encoded-device-length>-<encoded-device>/`, using the collision-free encoding in `contracts/failure-material.md`. Each device gets `driver.log`, `screenshot.png` or `screenshot-note.txt`, and its `data/` folder; the timeline records which device each step touched. The root of a failed device is kept under the same folder when `--keep` or a failure applies (it is small: SQLite files). `scenario.ts` (482 lines) is not allowed to grow past 500, so the per-device handling moves to a new file and `scenario.ts` only passes its device list.

## R6 The driver layer (FR-021, SC-005)

**Finding.** The Linux specifics are concentrated in `instance.ts` (Xvfb, `tauri-driver`, XDG directories, ports), `processes.ts` (signals, `/proc`), `webdriver.ts` and `build.ts`. Scenarios already mostly use `Page` operations; the exceptions are `copyFileSync` in `sync-copy-notice` and direct `ctx.nostrRelay()` handling.

**Decision.** One small interface, `DeviceHost`, whose operations are exactly those of FR-021 plus `copyVaultFile` and `kill`. The Linux implementation wraps the existing `instance.ts` and `processes.ts`; the group helpers and all scenarios depend only on the interface and on the types of `Page`. A check in `check:e2e-lib` scans scenarios and non-platform helpers and fails on imports of the platform files, `node:child_process`, `node:os`, `node:fs`, `process.kill` and the strings `xvfb`, `tauri-driver`, `/proc`. This is the smallest layer that makes SC-005 mechanically checkable; no second implementation is written now.

**Alternatives.** Abstracting `Page` as well (rejected: its operations are already driver-neutral in name and shape); a plugin registry (rejected: speculative).

## R7 Large sync for the lock scenario (M8)

**Finding.** The load test of spec 024 moves 1,000 changes in 16 s with induced aborts, but there is no bulk-create command.

**Decision.** Stop B, create N threads on A through `create_thread` (N chosen so the following sync takes at least about 5 s on a maintainer's machine), start B, wait until B holds some but not all threads, then lock A through the lock control. Check on B that A turns offline within the bound of spec 013's close promise plus the detection time of R1, reopen A and wait until B holds exactly N, none missing, none duplicated. **Gate (G3):** N is calibrated by measurement; if no N gives a window of at least 2 s on the stock CI runner, the scenario reports the overlap it achieved and the plan adds a bulk helper in the interface of the application's own tests, which is then a plan amendment.

## R8 Mutual removal (M7) and what a scenario can read

**Finding.** The winner rule (highest generation, then smallest list hash, `device_list.rs:275-285`) is unit-tested (`device_list_tests.rs:92,123`). No command exposes generation or hash; a scenario can read `sync_status.thisDevice` per device and `list_vault_devices`.

**Decision.** The scenario checks the invariants for any outcome: exactly one of the two main devices ends `main` and the other `removed` with `settings-federation-notice`, all devices list only the winner, the linked device still has a main device. The tie rule stays covered by the unit tests. The spec's FR-008 is aligned.

## R9 Other platforms (FR-022, document only)

To be confirmed in each follow-up spec; stated here so the interface of R6 fits them.

| Platform | Likely driver                                                                                     | Runner                       | Several devices                                           | Known limits                                                          |
| -------- | ------------------------------------------------------------------------------------------------- | ---------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------- |
| Windows  | Tauri WebDriver bridge (`tauri-driver`) with Microsoft Edge WebDriver                             | Windows runner               | several processes with separate data folders, as on Linux | no virtual screen; the screenshot and process operations differ       |
| macOS    | no official driver for the system web view; third-party driver or a bridge inside the application | macOS runner                 | separate data folders                                     | the biggest unknown; the bridge would be test-only code               |
| Android  | Appium (UiAutomator2) in the web view context                                                     | emulators or devices         | several emulators must reach each other on a network      | start and stop are app lifecycle, not processes; data is per emulator |
| iOS      | Appium (XCUITest)                                                                                 | macOS runner with simulators | several simulators on one host                            | simulators are limited in count; data is per simulator                |

The `DeviceHost` operations map to all four: `start`/`stop`/`kill` are app lifecycle calls, `goOffline`/`goOnline` are built from start, stop and the servers action, and `copyVaultFile` is a file transfer into the other device's sandbox.

## R10 CI

**Finding.** The `e2e` job has a 45 min limit and the command's default run limit is 600 s; the sync scenarios alone declare 240 to 360 s each.

**Decision.** Nine new scenarios plus the existing ones will exceed 600 s. The CI step passes `--run-timeout 2400`, the job limit rises to 60 min only if measurement shows it is needed, and the new scenarios keep their names starting with `sync-` so `--grep sync-` runs them alone. No tag mechanism is added.

## R11 Machine limit (FR-019)

Each device is an application process plus a virtual screen plus a driver. The largest scenarios (M6 and M7: two main devices, a linked device and a copy) need four. The group helper refuses more than `E2E_MAX_DEVICES` (default 6) at its start with a message naming the limit; the heavy scenarios are never run in parallel (`--test-concurrency=1` already).
