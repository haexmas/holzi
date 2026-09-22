# End-to-end tests

Starts the real, built Holzi binary under a virtual screen, drives it through `tauri-driver` the way a
user does, and checks what it does — without opening a window on your desktop and without touching your
own Holzi data. Background and design decisions: [`specs/016-e2e-testing/`](../../specs/016-e2e-testing/).

## Running

```sh
pnpm test:e2e
```

From the dev shell (`nix develop`), or through `scripts/with-nix-host-bridge.sh` directly, the way
`tauri:dev` and `tauri:build` run. This builds the debug application, starts a virtual screen per
scenario, runs every `scripts/e2e/scenarios/*.test.ts` file, prints a summary, and cleans up.

Common options:

| Option          | Meaning                                                                |
| --------------- | ---------------------------------------------------------------------- |
| `--app <path>`  | Test this already-built application instead of building one.           |
| `--grep <text>` | Run only scenarios whose name contains the text.                       |
| `--keep`        | Keep the run's material (screenshots, logs) for passing scenarios too. |

`--app` is for a release build: the ordinary run always builds and tests the debug profile (clarified in
`specs/016-e2e-testing/spec.md`); pointing `--app` at a release binary is the only way this suite tests
one. The full option and exit-status list is in
[`specs/016-e2e-testing/contracts/cli.md`](../../specs/016-e2e-testing/contracts/cli.md).

## The tools

`tauri-driver`, `WebKitWebDriver` and `xvfb-run` come from the Nix dev shell (holzi's atoms pin,
`.devshell/packages.nix`). A missing tool, or a driver whose WebKit version does not match the host's
`webkit2gtk-4.1`, stops the run before anything starts, with a message naming the tool or the versions
and the fix.

## Writing a scenario

Copy [`scenarios/create-and-unlock.test.ts`](scenarios/create-and-unlock.test.ts):

```ts
import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'

scenario('create-and-unlock', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  const name = 'e2e-test'

  await createAndUnlock(instance, { name })

  const result = await instance.invoke('list_instances')
  if (!('ok' in result) || !result.ok) {
    throw new Error(`list_instances failed: ${JSON.stringify(result)}`)
  }
  const names = (result.data as Array<{ name: string }>).map((i) => i.name)
  assert.ok(
    names.includes(name),
    `expected "${name}" among ${JSON.stringify(names)}`,
  )
  ctx.step('checked')
})
```

The file name and the string passed to `scenario(...)` must match (`pnpm test:e2e` checks this before it
starts anything). `node --test scripts/e2e/lib/*.test.ts` (`pnpm check:e2e-lib`) does not run scenario
files; only `pnpm test:e2e` does, since a scenario needs the built application and the tools.

## Helpers

The full contract, with every promise: [`contracts/helpers.md`](../../specs/016-e2e-testing/contracts/helpers.md).

On the context (`ctx`, the argument to a scenario's body):

| Member                     | What it does                                                                             |
| -------------------------- | ---------------------------------------------------------------------------------------- |
| `ctx.startInstance(opts?)` | A fresh, isolated running instance. Ended for you when the scenario ends.                |
| `ctx.provider(behavior?)`  | A stand-in model provider (see below). Ended with the context.                           |
| `ctx.step(name, detail?)`  | Adds a timeline entry, for the report and for diagnosing a failure without a second run. |
| `ctx.waitFor(desc, pred)`  | Polls until `pred()` is truthy or the deadline passes. Use this, never a fixed sleep.    |
| `ctx.credentials()`        | A generated passphrase and provider key, so none is ever committed.                      |

On an instance:

| Member                                       | What it does                                                                                                                                                                     |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `instance.invoke(command, args?, opts?)`     | Calls a backend command. `{ ok: true, data }`, `{ ok: false, error }`, or with `opts.expectEnd`, `{ ended: true }` for a call that gets no answer because the application ended. |
| `instance.click(hook)` / `.type(hook, text)` | Reach a control by hook ([below](#hooks)); use the displayed one if several match.                                                                                               |
| `instance.press(hook)`                       | Click a control that may close or navigate away, without waiting for a reply.                                                                                                    |
| `instance.exec(script, args?)`               | Runs a script on the page, for assertions on what it shows.                                                                                                                      |
| `instance.screenshot(name)`                  | Saves a PNG in the scenario's kept material.                                                                                                                                     |
| `instance.waitForEnd(deadlineMs)`            | Resolves with the time until the application process ends, or fails at the deadline.                                                                                             |
| `instance.markedProcesses()`                 | This run's processes running the application, for the relaunch and lock-twice checks.                                                                                            |

Convenience built on the above, in [`lib/flows.ts`](lib/flows.ts):

- `createAndUnlock(instance, { name })` — create a vault, unlock it, wait for the workspace.
- `openChat(instance)` — open the chat from the workspace.
- `connectProvider(instance, provider)` — point the instance at a stand-in provider and load its model.
- `startReply(instance, provider, text)` — send a message; waits until the provider sees it arrive.

## The stand-in provider

`ctx.provider(behavior?)` starts a local server that plays an external model provider of the Anthropic
kind — no application change, since the app already accepts any base URL for that kind of provider. Full
contract: [`contracts/stand-in-provider.md`](../../specs/016-e2e-testing/contracts/stand-in-provider.md).

```ts
const provider = await ctx.provider({ kind: 'stream-forever' }) // or 'stream-then-finish', 'error'
provider.behave({ kind: 'stream-then-finish', chunks: 3 }) // changes what the next request gets
await provider.waitForOpen() // resolves once a POST /v1/messages connection is open
provider.connections() // { id, openedAt, closedAt?, method, path }[]
```

`closedAt` is set from the socket's own close, so a reply the application cancelled shows up as a closed
connection, not as one that quietly finished.

## Hooks

Every control a scenario needs is reachable without depending on displayed text or the interface
language (the interface is German by default). Full table:
[`contracts/test-hooks.md`](../../specs/016-e2e-testing/contracts/test-hooks.md).

```ts
instance.click('lock-instance-sidebar') // [data-testid="lock-instance-sidebar"]
instance.click('#unlock-passphrase') // a selector starting with # . [ is used as is
```

A hook without a selector character is a `data-testid`. Add one only when a scenario needs it and
nothing else reaches the control (a `data-testid` and, where useful, a data attribute carrying the
control's own data — never its displayed text).

## Rules for scenarios

- Reach controls by hook only, never by displayed text or the interface language.
- Do not rely on a keyboard submit; click the button (pressing Enter did not submit the unlock form).
- Do not wait for the reply of a call that closes the application; pass `expectEnd`.
- Do not use a fixed sleep; use `ctx.waitFor` or the polling helpers above.
- Do not start a process outside the context, so the cleanup rules hold.
- A scenario that needs a relaunching build declares it: `scenario(name, { needs: { closeBehavior: 'relaunch' } }, …)`.
  It is skipped, not failed, against a build that does not relaunch.

## How the command and scenario files talk

`pnpm test:e2e` sets the environment before it spawns `node --test` over the scenario files:
`E2E_RUN_DIR`, `E2E_APP`, `E2E_CLOSE_BEHAVIOR`, `E2E_TOOLS` (JSON), `E2E_SCENARIO_TIMEOUT_MS`,
`E2E_TIME_SCALE`, `E2E_KEEP`, `HOLZI_E2E_RUN` (the run's process marker). `scripts/e2e/lib/scenario.ts`
reads it once, in `readEnv()`. A scenario file is not meant to be run any other way.

## The run directory

`<CARGO_TARGET_DIR or src-tauri/target>/e2e/<run-id>/` (moved with `E2E_ARTIFACTS_DIR`; already outside
version control). A passing run leaves only `report.json` and, if it built the application, `build.log`.
A failed or timed-out scenario also leaves its own directory: `timeline.json`, `screenshot.png` (unless
the application had already ended), `driver.log`, `provider.json`. `--keep` keeps a passing scenario's
directory too. Full shape: [`contracts/report.md`](../../specs/016-e2e-testing/contracts/report.md).

## Troubleshooting a leftover run

Every process the suite starts carries `HOLZI_E2E_RUN` in its environment, so a killed run's leftovers
are found and removed by the next one automatically — never by matching a process by name or path, so
your own Holzi is never touched. To check by hand:

```sh
grep -l HOLZI_E2E_RUN /proc/*/environ 2>/dev/null
```

Anything this prints beyond the current run is a leftover; the next `pnpm test:e2e` sweeps and reports it
before it builds or runs anything.
