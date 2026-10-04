# Quickstart: validating spec 033

Prerequisites: the Nix development shell, a real `pnpm install` in the worktree (no symlinked `node_modules`), `tauri-driver` and the WebKit driver from the shell (spec 016).

## Automatic

| Step                  | Command                                                                                               | Expected                                                                                                                   |
| --------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| V1 Helper checks      | `pnpm check:e2e-lib`                                                                                  | group, host interface, seam scan and failure-material layout pass with no display                                          |
| V2 Scripts type check | `pnpm typecheck:scripts`                                                                              | no error                                                                                                                   |
| V3 Seam scan          | part of V1                                                                                            | no scenario or non-platform helper uses a platform specific (SC-005)                                                       |
| V4 The nine scenarios | `nix develop --command scripts/with-nix-host-bridge.sh pnpm test:e2e --grep sync-`                    | all `sync-*` scenarios pass, including the nine from M1 to M9                                                              |
| V5 Whole suite        | `nix develop --command scripts/with-nix-host-bridge.sh pnpm test:e2e`                                 | every scenario passes, no process of the run is left                                                                       |
| V6 Leftovers          | run V4 twice in a row                                                                                 | the second run starts at once and its start-up sweep reports no leftovers of the first (SC-006)                            |
| V7 Deliberate failure | break the sync on purpose (for example make `device_remove` do nothing) and run the matching scenario | the scenario fails, names the device in the step, and keeps each encoded device folder's `screenshot.png` and `driver.log` |
| V8 Size               | one scenario with two users and two devices each, written from the template                           | 60 lines or fewer (SC-003)                                                                                                 |

## Gates (research)

- **G1** relay returns on the same URL and the application's client reconnects (Stage 2; blocks the Stage 2 gate if it fails).
- **G2** link through the form works on loopback with default iroh relays (Stage 2).
- **G3** a data volume exists whose sync window is at least 2 s on the stock runner (Stage 3).
- **G4** how a running device goes offline (Stage 1): decided on 2026-10-02, restart with no servers; result in research R2.

## Manual

None. The manual section of the quickstart of spec 024 is replaced by a pointer to V4 once SC-002 holds (each of the nine scenarios of V4 passes in at least 19 of the 20 most recent CI runs on `main`, FR-011).
