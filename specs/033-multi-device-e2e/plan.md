# Implementation Plan: End-to-End Tests Across Several Vaults and Devices

**Branch**: `033-multi-device-e2e` (spec merged in #199; this plan is on `033-plan`) | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/033-multi-device-e2e/spec.md`, clarified on 2026-10-01 (Linux only, general building blocks for later features).

## Summary

The nine manual checks of spec 024 become nine automatic scenarios on the rig of spec 016, driven through the real interface. To write them, the rig gets a small group layer (`group.ts`) that starts users with several devices, links them, takes a device offline and back, copies a vault file, reads the device list and online state, and ends everything with the scenario. A new interface, `DeviceHost`, is the only thing that knows the platform; its Linux implementation wraps the existing code unchanged. A scan in the helper checks proves that no scenario uses a platform specific (SC-005).

Research changed three things the spec assumed. The online state is not a timer in the application, so no clock control is built: the 60 s of M4 is a fixed deadline for noticing a vanished peer (R1). A running device is taken offline by restarting it with no servers, because the in-place alternative (network namespaces) works on a development machine but needs a rig rework and rights the stock runner does not give by default (R2, gate G4). The test relay gets an optional port so it can return on the same URL (R3). The application changes by one `data-testid` (the form's submit button); the Rust side by a port argument of the relay test binary.

## Technical Context

**Language/Version**: TypeScript run by Node.js 22.19 or later with type stripping, as the rig of spec 016. A few lines of Rust in `src-tauri/src/bin/e2e_nostr_relay.rs`. One attribute in a Vue component.

**Primary Dependencies**: None new. Node built-ins and the tools of spec 016 (`tauri-driver` 2.0.6, `WebKitWebDriver`, `xvfb-run`).

**Storage**: Files only, under the run directory `<target>/e2e/<run-id>/`; device data folders there, removed at the end unless kept.

**Testing**: Scenarios `scripts/e2e/scenarios/*.test.ts` run by `node --test`; the helpers' logic has checks in `scripts/e2e/lib/*.test.ts` (`pnpm check:e2e-lib`, display-free): group planning, state transitions, the seam scan, failure-material layout, the device limit, copy helper against a fake host.

**Target Platform**: Linux (maintainer's CachyOS with the Nix shell; `ubuntu-24.04` in CI). Other platforms are documented in `scripts/e2e/PLATFORMS.md`, not implemented.

**Project Type**: Desktop application; the feature is test tooling in `scripts/e2e/`.

**Performance Goals**: the slowest scenario under 3 minutes on a maintainer's machine, M4 under 90 s (SC-004); a deliberately broken sync fails the job and names the scenario (SC-008).

**Constraints**: no window on the desktop, no network beyond loopback, exact process identification (spec 016); at most `E2E_MAX_DEVICES` (default 6) devices per scenario; files under 500 lines (`scenario.ts` is at 482 and must not grow); tests in dedicated files; no fixed sleeps.

**Scale/Scope**: nine new scenarios, about six new small library files, one changed hook, one changed test binary, one CI line.

## Constitution Check

_GATE: passed before research; re-checked after design._ Sources: the project constitution (principles I to VIII, workflow) and the spaex behavior harness (`.spaex/constitution.md`).

| Rule                                             | Status           | How                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------------ | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I No secrets in git                              | Pass             | Passphrases are random per run; no literal credential.                                                                                                                                                                                                                                                                                             |
| II No local absolute paths                       | Pass             | All paths come from the run directory and the target directory at run time; docs use relative paths.                                                                                                                                                                                                                                               |
| III, IV, V, VI, VIII                             | N/A or Pass      | No external source, pin or instruction file changes. `PLATFORMS.md` names tools by name and version family, with no local path.                                                                                                                                                                                                                    |
| VII Relay unavailability never blocks local work | Pass, and tested | Scenarios M7 and M9 check exactly this.                                                                                                                                                                                                                                                                                                            |
| Tests in dedicated files                         | Pass             | Scenarios and checks are `*.test.ts`; helpers are never in a test file.                                                                                                                                                                                                                                                                            |
| 500-line boundary                                | Tracked          | `scenario.ts` (482) gets only a wiring change; new code goes into `group.ts`, `platform/*.ts`, `artifacts.ts` additions stay small. See Complexity Tracking.                                                                                                                                                                                       |
| Graphify before authoring named artifacts        | Planned          | Before each new helper, run `graphify query` for the candidates found in the survey: `linkDevice`, `startFirstDevice`, `restartDevice`, `closeDevice`, `onlyServers`, `expectThreads` (`sync-flows.ts`), `captureFailure`, `startNostrRelay`. The group layer extends these and does not duplicate them. The result goes into the implementing PR. |
| `ponytail:` comments on shortcuts                | Planned          | At the restart-based offline (resets in-memory addresses), the fixed device limit, and the calibrated data volume.                                                                                                                                                                                                                                 |
| One runnable check per non-trivial logic         | Pass             | `lib/*.test.ts` with the adopted `node:test`.                                                                                                                                                                                                                                                                                                      |
| No `unwrap`/`expect` on I/O                      | Pass             | Rust change is a port argument read with an error path, not `unwrap`.                                                                                                                                                                                                                                                                              |
| ADR for decisions affecting a principle          | N/A              | No principle changes.                                                                                                                                                                                                                                                                                                                              |
| Conventional Commits, no agent reference         | Planned          | Applied when committing.                                                                                                                                                                                                                                                                                                                           |
| Pull request into `main`, no squash              | Planned          | Stages land as pull requests.                                                                                                                                                                                                                                                                                                                      |
| No wall-clock dependence in tests (Rust rule)    | Excepted         | These scenarios measure real time by design; waits are polls with fixed deadlines, never fixed sleeps.                                                                                                                                                                                                                                             |

Post-design re-check: no new violation. The restart-based offline (R2) keeps production code free of test switches, which the first idea (a test command in the application) would not have.

## Project Structure

### Documentation (this feature)

```text
specs/033-multi-device-e2e/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── group.md
│   ├── driver-layer.md
│   └── failure-material.md
├── checklists/requirements.md
└── tasks.md            # by speckit-tasks
```

### Source code

```text
scripts/e2e/
├── PLATFORMS.md                     # new: what each further platform needs (FR-022)
├── README.md                        # extended: group helpers, template, rules
├── lib/
│   ├── group.ts                     # new: Group, User, Device, goOffline/goOnline, expectations
│   ├── group.test.ts                # new: planning, limit, transitions with a fake host
│   ├── platform/
│   │   ├── host.ts                  # new: DeviceHost, RunningDevice, DataHandle
│   │   └── linux.ts                 # new: implementation over instance.ts, processes.ts
│   ├── platform/linux.test.ts       # new: copyVaultFile consistency, kill, refusal on running source
│   ├── seam.test.ts                 # new: scan of scenarios and non-platform helpers (SC-005)
│   ├── sync-flows.ts                # changed: linkDevice gains the form route; helpers take Device
│   ├── nostr-relay.ts               # changed: fixed port, stop and start again
│   ├── artifacts.ts                 # changed: per-device material (see contract)
│   └── scenario.ts                  # changed: ctx.group wiring only
├── scenarios/
│   ├── sync-link.test.ts            # changed: M1 through the start-page form
│   ├── sync-two-devices.test.ts     # changed: M2 through the interface
│   ├── sync-identity.test.ts        # new: M3
│   ├── sync-presence.test.ts        # new: M4
│   ├── sync-copy-notice.test.ts     # replaced by sync-copy.test.ts: M5, no fs access
│   ├── sync-remove-device.test.ts   # new: M6
│   ├── sync-mutual-removal.test.ts  # new: M7
│   ├── sync-lock-during-sync.test.ts# new: M8
│   └── sync-servers-off.test.ts     # new: M9
src-tauri/src/bin/e2e_nostr_relay.rs # changed: optional port
src/components/onboarding/LinkSheet.vue # changed: data-testid="link-submit"
.github/workflows/ci.yml             # changed: --run-timeout for the e2e step
specs/024-own-device-sync/quickstart.md # changed at the end: manual section replaced by a pointer (FR-011)
specs/024-own-device-sync/tasks.md   # changed at the end: T081 closed
```

**Structure Decision**: Everything stays in the existing `scripts/e2e/` tree of spec 016. The platform seam is one new folder, `lib/platform/`, with the interface and the one Linux file; nothing outside it may name a platform specific.

## Stages

| Stage                                          | Content                                                                                                                                                  | Gate / proof                                                                         |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 0 Spec alignment                               | FR-008 (tie rule is unit-tested; scenario checks the invariants) and FR-014 (no clock control, fixed deadline) aligned to research R8 and R1             | spec re-checked, checklist unchanged                                                 |
| 1 Foundations                                  | `platform/host.ts` and `linux.ts` over the existing code; `group.ts`; per-device failure material; relay with a fixed port; seam scan; limit; lib checks | `pnpm check:e2e-lib` green; all existing scenarios still pass unchanged              |
| 2 Linking, sync, identity, presence (M1 to M4) | `link-submit` hook; four scenarios, two rewritten                                                                                                        | **G1 and G2**; a deliberately broken sync makes each fail with device-named material |
| 3 Copies, removal, lock, servers (M5 to M9)    | five scenarios; `sync-copy-notice` retired in favour of `sync-copy`                                                                                      | **G3**; 20 consecutive green runs on the maintainer's machine                        |
| 4 CI, retire the manual checks                 | `--run-timeout` in CI; `PLATFORMS.md`; README; manual section of the quickstart of spec 024 replaced; T081 closed                                        | SC-002 (20 consecutive CI runs), SC-003 (60-line reviewer test), SC-007, SC-008      |

Stages 1 to 4 each land as a pull request. Stage 2 and 3 scenarios are independent of each other once Stage 1 is in.

## Spec alignment

Two items of the spec change with this plan, because research showed the first wording cannot be met as written:

- **FR-014** asked for control of the waiting time. The online state has no clock threshold in the application (R1). It now asks for fixed deadlines equal to the promise, documented, with no clock control; SC-004 is unchanged.
- **FR-008** asked the scenario to check that the winner is the one with the smaller list hash. No command exposes the hash (R8). It now asks for the invariants for any outcome; the tie rule stays covered by the unit tests of `device_list`.

## Complexity Tracking

| Item                                                     | Why needed                                                               | Simpler alternative rejected because                                                                                                                              |
| -------------------------------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scenario.ts` at 482 lines                               | Needs a wiring line for `ctx.group`                                      | Putting group code in it would cross 500; it goes to `group.ts`. If the wiring crosses 500, extract `ScenarioContext` first, as a separate, behavior-free commit. |
| An interface with one implementation (`DeviceHost`)      | The spec requires the seam and a mechanical check of it (FR-021, SC-005) | Leaving the Linux code addressed directly would make the follow-up specs rewrite scenarios. The interface is the operations list of FR-021 and nothing more.      |
| Possible debug-build override of the default iroh relays | Only if Gate G2 fails                                                    | The form cannot set iroh relays; the alternative is a product change to the form. Decided at G2 with a reviewed plan amendment.                                   |
