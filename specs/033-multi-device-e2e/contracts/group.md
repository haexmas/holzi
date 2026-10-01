# Contract: group helpers

Module `scripts/e2e/lib/group.ts`, built on `lib/sync-flows.ts` (which keeps the single-purpose helpers) and the `DeviceHost` of the driver layer. Scenarios import only this module, `scenario.ts`, `page.ts`-level types and `sync-flows.ts`. Signatures show intent; exact types are fixed in implementation.

## Creating a group

```ts
const g = await ctx.group({
  users: { anna: ['laptop', 'phone'], ben: ['desktop'] }, // first device of each user creates the vault
  relay: 'shared', // one relay for all users (default)
})
const laptop = g.device('anna/laptop')
```

- Starts the relay, creates each user's vault on its first device (servers set to the group relay only), and links the other devices of a user the way `linkDevice` does today (commands, not the form). Passing `link: 'form'` for a device links it through the start-page form instead (used by the M1 scenario).
- Refuses at once, with a message naming `E2E_MAX_DEVICES`, a group with more devices than the limit.
- Everything started is ended when the scenario ends, in reverse order, whether it passed, failed or timed out.

## Device operations

| Call                                        | Effect                                                                                                                                  |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `device.page`                               | the `Page` of spec 016 (click, type, invoke, exec, screenshot, waitForDisplayed, navigate)                                              |
| `device.stop()`                             | graceful end, data kept                                                                                                                 |
| `device.kill()`                             | end without cleanup (SIGKILL on Linux), data kept                                                                                       |
| `device.start()`                            | start over the same data and open the vault                                                                                             |
| `device.restart()`                          | `stop()` then `start()`                                                                                                                 |
| `device.goOffline()`                        | set servers to none, restart; the device runs and works locally but cannot reach or be reached                                          |
| `device.goOnline()`                         | set the group's servers back, restart                                                                                                   |
| `device.setNostrRelays(urls, { disabled })` | the settings action of the servers view, through the interface                                                                          |
| `device.deviceList()`                       | the rows the device view shows: `{ name, role, current, online, lastSeen, text }`                                                       |
| `device.status()`                           | `sync_status` of the device                                                                                                             |
| `device.identity()`                         | `{ npub, hex }`                                                                                                                         |
| `device.copyVaultTo(name, { user })`        | copy this device's vault file consistently to a new device `name` of the group (not started); reports failure instead of a partial copy |

## Group operations

| Call                                 | Effect                                                                                                                     |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| `g.relay.stop()` / `g.relay.start()` | switch the relay off and on again on the same URL                                                                          |
| `g.expect(...)`                      | wait helpers with fixed deadlines (`expectThreads`, `expectOnline`, `expectLastSeen`, `expectRole`) built on `ctx.waitFor` |

## Waiting

Waits that depend on sync use fixed deadlines (`fixed: true`) that `E2E_TIME_SCALE` does not stretch, as in spec 016 and the existing sync helpers. The 60 s of M4 is one such deadline (research R1).

## Not allowed in scenarios

Imports of `instance.ts`, `processes.ts`, `webdriver.ts`, `build.ts`, `node:child_process`, `node:os`, `node:fs`; calls to `process.kill`; the strings `xvfb`, `tauri-driver`, `/proc`. A check in `check:e2e-lib` enforces it (SC-005).
