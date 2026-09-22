---
description: 'Task list for End-to-End Testing of the Real Application'
---

# Tasks: End-to-End Testing of the Real Application

**Input**: Design documents from `specs/016-e2e-testing/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/cli.md, contracts/helpers.md,
contracts/stand-in-provider.md, contracts/report.md, contracts/test-hooks.md, quickstart.md

**Tests**: Included as mandatory, not optional. The spaex constitution requires test code in files
separate from production code and one runnable check for every piece of non-trivial logic. Two kinds
exist here and both live in dedicated files. The helpers' own checks are `scripts/e2e/lib/*.test.ts`,
run by `pnpm check:e2e-lib` with no display and no application. The scenarios are
`scripts/e2e/scenarios/*.test.ts`, run only by `pnpm test:e2e`. Test-only helpers are files ending in
`.testlib.ts`, never mixed into production files. No new test runner and no new dependency: Node's
built-in `node:test`.

**Organization**: The plan's delivery stages, in the plan's order, each a pull request that can be
reviewed and used on its own. The story labels are US1 run the suite safely, US2 the spec 013 close
promises, US3 a new scenario is short, US4 a failed scenario explains itself, US5 missing tools stop the
run early, US6 the CI job. Stage 1 delivers US1 and US5 together because the command's preflight belongs
to the command; Stage 2 delivers US3 and US4; Stage 3 delivers US2, which needs what Stage 2 builds. The
two P1 stories (US1 and US2) with their prerequisites are the MVP.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on another unchecked task)
- **[Story]**: US1 to US6 (only in story phases; the setup, foundational, validation and polish phases
  have none)
- All paths are repository-relative and written in full: `scripts/e2e/...`, `src/...`, `.github/...`.
- Commits follow Conventional Commits, one per checkpoint, **without** any agent or model trailer
  (constitution MUST NOT). Work lands on `main` through a pull request per stage, from a topic branch;
  no squash-merge. Opening a PR and pushing are done only after asking the operator.
- Inside the Nix dev shell: `nix develop --command pnpm ...` for scripts and the frontend, and
  `nix develop --command scripts/with-nix-host-bridge.sh ...` for anything that starts the application
  or needs the host's web view. `pnpm test:e2e` already runs through the bridge.
- **No credential literal in any committed file**, not even a test value: passphrases and provider keys
  are generated per run (constitution I). **No absolute or home-relative path in any committed file**
  (constitution II): derive paths at run time.
- **Marked shortcuts**: a deliberate shortcut with a known ceiling gets a `ponytail:` comment naming the
  ceiling and the upgrade path. Required at: the free-port choice (`scripts/e2e/lib/ports.ts`), the
  `/proc` scan (`scripts/e2e/lib/processes.ts`), each poll loop with a fixed deadline, and the one
  virtual screen per instance (`scripts/e2e/lib/instance.ts`).
- **Files stay under 500 lines.** `src/pages/chat/[instance].vue` (1316 lines) may gain exactly the two
  attribute lines of T048 and nothing else. No unwrap-like shortcuts on I/O: catch at the boundary that
  can report it and keep the cause in the message.
- **Graphify first**: before authoring a new named function, class, module or command in a stage, run
  the graphify queries in that stage's first task, from the primary checkout (read-only inspection is
  allowed; a linked worktree has no graph of its own) and note in the PR description which candidates
  were evaluated and why none was extended.
- Waits are polls with a deadline, never fixed sleeps. Generic scenario and run timeouts may be multiplied
  by `E2E_TIME_SCALE` (default 1); the 4-second, 1-second and 10-second close promises remain fixed for
  conformance. A run with a scale other than 1 is marked non-conformant and reports the scale.
- Contract between the command and the scenario files, by environment: `E2E_RUN_DIR`, `E2E_APP`,
  `E2E_CLOSE_BEHAVIOR`, `E2E_TOOLS` (JSON of resolved tool paths), `E2E_SCENARIO_TIMEOUT_MS`,
  `E2E_TIME_SCALE` and `HOLZI_E2E_RUN` (the marker). Each is written once by `cli.ts` (T029) and read
  once by `scenario.ts` (T021).

---

## Phase 1: Setup — Stage 0, the tools reach the dev shell (FR-012)

**Purpose**: haexmas/atoms#32 is merged as `d5c48d0eb6662da84086bf69a3ca041a494e2b26`; holzi still pins
the older revision, so the shell has no `tauri-driver`, no `WebKitWebDriver` and no `xvfb-run`.

- [x] T001 Work in `.worktrees/016-e2e-testing` with a real `pnpm install` (no symlinked
      `node_modules`). This branch, `016-e2e-testing-plan`, carries the plan and these tasks; each
      later stage starts from the updated `origin/main` on its own branch (`016-e2e-stage0-tools`,
      `016-e2e-stage1-command`, `016-e2e-stage2-helpers`, `016-e2e-stage3-close`,
      `016-e2e-stage4-validation`, `016-e2e-stage5-ci`), created after the previous stage merged.
- [x] T002 Record the baseline in the "Baseline" section at the end of this file before any edit:
      `pnpm typecheck:scripts`, `pnpm lint`, `pnpm format:check`, `pnpm check:chat-state`,
      `pnpm check:vault-lifecycle`, `pnpm check:templates` results, and `wc -l` of
      `src/pages/chat/[instance].vue` (1316 at planning time), `src/components/onboarding/InstancesList.vue`
      (44) and `src/components/workspace/ChatFab.vue` (20).
- [x] T003 In a clone of `https://github.com/haexmas/atoms`, list what arrives with the new pin:
      `git log --oneline 1a292194da6a279eb382e562870fed1c41ebdf8e..d5c48d0eb6662da84086bf69a3ca041a494e2b26`.
      Put the list in the PR description. Anything besides #32 that changes a delivered file is noted
      there and checked in T005.
- [x] T004 In `.spaex/manifest.json` change the `revision` of the entry whose `source` is
      `https://github.com/haexmas/atoms` from `1a292194da6a279eb382e562870fed1c41ebdf8e` to the full
      hash `d5c48d0eb6662da84086bf69a3ca041a494e2b26` (a branch or `HEAD` is not allowed for a pin,
      constitution IV; the old value occurs once). Then run `spaex install` from the worktree root.
- [x] T005 Review the diff `spaex install` produced. Expected: `flake.nix` (now reads
      `.devshell/packages.nix`), the new `.devshell/packages.nix`, `.spaex/generated/nix-packages.json`
      (gains `xvfb-run`) and the manifest. Any other changed file is listed in the commit body and
      justified or reverted. No absolute path or secret may appear in a delivered file. Confirm
      `git ls-files .devshell/packages.nix` lists it (the flake reads only tracked files).
- [x] T006 Verify the tools in the shell. Run the block below through
      `nix develop --command scripts/with-nix-host-bridge.sh bash -c '<block>'`. Each tool prints a path
      in the Nix store and the two versions are equal (2.52.6 at planning time). The first
      `nix develop` after the pin builds `tauri-driver` from source and takes several minutes. Record
      the output in the "Validation record".

      ```bash
      command -v tauri-driver WebKitWebDriver xvfb-run
      cat "$(dirname "$(dirname "$(command -v WebKitWebDriver)")")/share/webkit-webdriver/version"
      pkg-config --modversion webkit2gtk-4.1
      ```

- [x] T007 Commit `chore(spaex): pin atoms at the revision that delivers the e2e tools` (body: the T003
      list and the T005 review). Push and open the Stage 0 PR after asking the operator.

**Checkpoint**: the shell provides the three tools and the version file; nothing else changed.

---

## Phase 2: Foundational — process control, driver client, instance, scenario core (Stage 1, blocks everything)

**Purpose**: what every story needs to start, drive and clean up an application. Nothing here runs the
application yet.

- [x] T008 Start branch `016-e2e-stage1-command` from the updated `origin/main`. Graphify first, from the
      primary checkout: `graphify query "spawn a child process and wait until it is ready"`,
      `graphify query "temporary directory created and removed by a test"`,
      `graphify query "http client for a json wire protocol"`,
      `graphify query "read environment of a process"`. Evaluate every candidate, including unexported
      ones; expected outcome (plan, research R9): none can be extended.
- [x] T009 In `package.json` add `"test:e2e": "scripts/with-nix-host-bridge.sh node scripts/e2e/cli.ts"` and
      `"check:e2e-lib": "node --test scripts/e2e/lib/*.test.ts"`. Confirm on Node 22.19 that
      `node --test` runs `.ts` files by type stripping without a flag and that
      `pnpm typecheck:scripts` (whose `tsconfig.scripts.json` includes `scripts/**/*.ts`) type-checks the
      new folder, including `node:test` types. No dependency is added.

### Tests for the foundation (write first; expected to fail until the implementation tasks land)

- [x] T010 [P] `scripts/e2e/lib/processes.test.ts`. Linux only: skip with a reason elsewhere. Cases:
      the marker is `<runner pid>:<runner start time>:<random>` and parses back, the start time read
      from field 22 of `/proc/self/stat`; a child spawned with the marker is found by the scan and one
      without is not; the application process is the marked one whose `/proc/<pid>/exe` equals the given
      binary (a marked process with another executable is not); a marker whose owner (pid and start
      time) is gone, or whose pid exists with a different start time, is an orphan, while the own run
      and another live runner are not; `sweepOrphans` stops only orphans and reports them, never an
      unmarked process; `stopRun(marker)` stops every process of that marker; `stopGroup` stops a
      detached group including a grandchild. Use short-lived `sleep` children; no fixed sleeps.
- [x] T011 [P] `scripts/e2e/lib/ports.test.ts`: `freePort()` returns an integer from 1024 to 65535 that can
      be bound at once; `withPortRetry` retries exactly once when the operation fails with an
      address-in-use error and rethrows the second failure with the port in the message.
- [x] T012 [P] `scripts/e2e/lib/webdriver.test.ts` and the test-only helper
      `scripts/e2e/lib/fake-driver.testlib.ts`: a local HTTP server speaking the W3C calls the client
      uses (new session, delete session, execute sync and async, find element, element click, element
      value, screenshot, close window, navigate). Cases: the new-session body is
      `{ capabilities: { alwaysMatch: { 'tauri:options': { application } } } }`; the script timeout is
      set to 60000 at session start; an element id is read from the key
      `element-6066-11e4-a52e-4f735466cecf`; `invoke` resolves `{ ok: true, data }` or
      `{ ok: false, error }`; when the server drops the connection or answers 500 during a call, `invoke`
      with `expectEnd` resolves `{ ended: true }` and without it throws an error that names the command.
- [x] T013 [P] `scripts/e2e/lib/instance.test.ts`, pure parts only (no real tools): `buildInstanceEnv` puts
      `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR` and `HOME` inside the
      instance root, sets `DBUS_SESSION_BUS_ADDRESS` to `disabled:` and `HOLZI_E2E_RUN` to the marker,
      and does not pass through the parent's `XDG_*` or `HOME` (set sentinels in the test); `prepareRoot`
      creates `data`, `config`, `cache`, `run` (mode 0700) and `home` empty and writes
      `config/gtk-3.0/settings.ini` with `gtk-application-prefer-dark-theme=true` for `dark` and `=false`
      for the default `light`; `removeRoot` deletes the tree; with `reusesRoot` the tree is not emptied;
      `driverCommand` yields `xvfb-run -a -s "-screen 0 1280x800x24" tauri-driver --port <p> --native-port <n> --native-driver <path>`
      with the given ports and paths.
- [x] T014 [P] `scripts/e2e/lib/scenario.test.ts` with a fake instance factory: `needs.closeBehavior`
      different from the application's means the body never runs and the result is `skipped` with the
      reason "the build exits on close; this scenario needs one that relaunches" (and its reverse);
      a throwing body gives `failed` and every registered teardown ran exactly once, in reverse order; a
      body that never resolves with `timeoutMs` 100 gives `failed` with a message naming the deadline,
      the teardown ran and the next scenario still runs; `timeoutMs` defaults to 60 000 and
      `E2E_TIME_SCALE=2` doubles it; `ctx.waitFor` polls (interval at most 50 ms) until true and on
      timeout the error contains the description and the last observed value; `ctx.step` records
      `atMs` from one monotonic clock; the result is written to `<E2E_RUN_DIR>/results/<name>.json`
      with `name`, `status`, `durationMs`, `skipReason` and `steps`; a duplicate scenario name is an
      error.
- [x] T015 [P] `scripts/e2e/lib/preflight.test.ts`, tool resolution only for now: `resolveTools` returns
      the absolute path of `tauri-driver`, `WebKitWebDriver` and `xvfb-run` for those present on a given
      `PATH` (temporary directory with executable stubs) and lists the missing ones; a directory or a
      non-executable file of that name does not count; no shell is used.

### Implementation for the foundation

- [x] T016 [P] `scripts/e2e/lib/processes.ts`: `newMarker`, `parseMarker`, `scanMarked` (reads
      `/proc/<pid>/environ`, same user only, unreadable entries skipped), `findApplicationProcesses`,
      `sweepOrphans`, `stopRun`, `stopGroup`, `spawnMarked` (detached, own process group, output to a
      log file with a timestamp per line). Identification is by marker and executable only, never by
      name or command line (FR-008). `ponytail:` at the scan: linear in the number of processes.
- [x] T017 [P] `scripts/e2e/lib/ports.ts`: `freePort` (bind port 0, release, return) and `withPortRetry`.
      `ponytail:` at `freePort`: the port can be taken between release and use; one retry, upgrade path is
      to pass an already bound socket if the driver ever supports it.
- [x] T018 [P] `scripts/e2e/lib/webdriver.ts`: the minimal W3C client over global `fetch`, with the calls
      listed in T012 and `invoke` as an async script that calls `window.__TAURI_INTERNALS__.invoke` and
      resolves to an object instead of throwing. No dependency.
- [x] T019 [P] `scripts/e2e/lib/preflight.ts`: `resolveTools(pathEnv)` and the tool names as constants.
      The version check follows in T034; keep the file's exports so T034 only adds.
- [x] T020 `scripts/e2e/lib/instance.ts` (after T016 to T019): `buildInstanceEnv`, `prepareRoot`,
      `removeRoot`, `driverCommand`, `startInstance` (free ports, spawn marked with an own virtual
      screen, wait for the driver port with a deadline, open the session, find `appPid` as the marked
      process whose executable is the application, record `instance-ready`), `stop`, and for now
      `exec`, `screenshot`, `alive`. The instance root is "Created empty at start, removed at the end
      (FR-006). Holds `data`, `config`, `cache`, `run` and `home`", and the environment is exactly the
      set in data-model.md. `colorScheme` is "`light` or `dark`, default `light`". `ponytail:` at the
      one virtual screen per instance. If the file nears 400 lines, split the session code into
      `scripts/e2e/lib/session.ts`.
- [x] T021 `scripts/e2e/lib/scenario.ts` (after T020): `scenario(name, options, body)` on top of `node:test`,
      the context (`ctx.app`, `ctx.startInstance`, `ctx.step`, `ctx.waitFor`, `ctx.credentials` with a
      passphrase and a provider key generated per run), deadline, teardown in reverse order, the skip
      rule for `needs`, and the result file. Reads the environment listed in the Format section.
      Failure material is added in T057, so keep an `onFailure` extension point.
- [x] T022 In `.github/workflows/ci.yml`, documentation job, add after the "Check vault lifecycle" step:
      a step "Check e2e helpers" running `corepack pnpm check:e2e-lib`, with a comment saying it needs
      no display and no application.
- [x] T023 Checkpoint: `pnpm check:e2e-lib`, `pnpm typecheck:scripts`, `pnpm lint`, `pnpm format:check`
      pass. Commit `feat(e2e): process control, driver client and isolated instances`.

**Checkpoint**: the foundation is tested in isolation; nothing runs the application yet.

---

## Phase 3: User Story 1 — Run the whole suite with one command, safely (Priority: P1) 🎯 MVP

**Goal**: One command from the dev shell builds, runs, reports and cleans up, without a window on the
desktop and without touching the maintainer's Holzi or its data (FR-001 to FR-005, FR-008 to FR-010).

**Independent Test**: Run the command with the smoke scenario only. Afterwards no window appeared, no
marked process survives, the data of a Holzi that was running at the time is unchanged, and that Holzi
kept running.

### Tests for User Story 1 (write first)

- [x] T024 [P] [US1] `scripts/e2e/lib/build.test.ts`: the target directory is `CARGO_TARGET_DIR` if set, else
      `src-tauri/target`, and the binary `<target>/debug/holzi`; `--app` skips the build and must exist and
      be executable; the close behavior is `--close-behavior` if given (`closeBehaviorFrom: flag`), else
      `relaunch` for a path containing `/release/` and `exit` for `/debug/` (`closeBehaviorFrom: path`),
      else an error that says how to state it with `--close-behavior`; a build failure error carries the
      last 40 lines of the build output and the path of `build.log`.
- [x] T025 [P] [US1] `scripts/e2e/lib/report.test.ts`: from result files the summary prints one line per
      scenario with status, name and duration, and skipped scenarios with their reason; the run status is
      `passed` only if no scenario failed; a scenario file that ended without writing a result counts as
      `failed` with the message "no result written"; the exit statuses are the table of contracts/cli.md
      (0 no scenario failed, 1 failed or timed out, 2 preflight failed, 3 build failed, 130 interrupted,
      143 terminated); `report.json` has the fields of contracts/report.md.
- [x] T026 [P] [US1] `scripts/e2e/lib/cli.test.ts` with injected steps: the order is preflight, sweep,
      build (unless `--app` or `E2E_APP`), scenarios, sweep by marker, summary; a failing preflight
      starts nothing and exits 2; a failing build exits 3 and runs no scenario; `--grep` becomes the
      runner's name filter; SIGINT and SIGTERM stop the run's processes first and exit 130 and 143;
      `--run-timeout` stops the current scenario as a failure, lists the rest as skipped with the reason
      "run time limit" and exits 1; option defaults are scenario limit 60 s and run limit 600 s; every
      file `scripts/e2e/scenarios/<name>.test.ts` declares `scenario('<name>'`.

### Implementation for User Story 1

- [x] T027 [US1] `scripts/e2e/lib/build.ts`: `resolveApplication` and `buildDebugApplication`
      (`pnpm tauri build --debug --no-bundle` in the repository root, default features, output to
      `build.log` in the run directory). It never builds a release profile (FR-004, clarified 2026-09-21).
- [x] T028 [US1] `scripts/e2e/lib/report.ts` (after T025): read the result files, print the summary,
      write `report.json` with the run marker and conformance status/scale, and map the outcome to the exit
      status.
- [x] T029 [US1] `scripts/e2e/cli.ts` (after T027, T028): options `--app`, `--close-behavior`, `--grep`,
      `--keep`, `--scenario-timeout`, `--run-timeout` and the environment `E2E_APP`, `E2E_ARTIFACTS_DIR`,
      `E2E_TIME_SCALE`; run id from a timestamp plus a random suffix; the order of work of
      contracts/cli.md; it runs `node --test --test-concurrency=1` over `scripts/e2e/scenarios/*.test.ts`
      in a marked, detached group with the environment of the Format section; signal handling and the
      final sweep run on every path. The preflight call is a stub until T035 (tool resolution only).
- [x] T030 [US1] `scripts/e2e/scenarios/smoke-start.test.ts`: start an instance, wait until
      `document.readyState` is `complete` and `location.pathname` is `/`, take a screenshot, end. No
      hook and no text is used.
- [ ] T031 [US1] Manual verification of the independent test, recorded in the "Validation record": start
      your own Holzi (for example `pnpm tauri:dev`) and note a checksum listing of its data directory;
      run `pnpm test:e2e --grep smoke`; then check no window appeared on the desktop, no process with a
      run marker remains (`grep -l HOLZI_E2E_RUN /proc/*/environ 2>/dev/null` prints nothing), the
      listing is unchanged and your Holzi still runs. Repeat with Ctrl-C at a random moment, and with
      `kill -9` on the runner followed by a second run: the second run's sweep reports and removes the
      leftovers, and the run passes.
- [x] T032 [US1] Checkpoint: `pnpm check:e2e-lib`, `pnpm typecheck:scripts`, `pnpm lint`,
      `pnpm format:check`. Commit `feat(e2e): run the suite with one command`.

**Checkpoint**: one command runs the smoke scenario safely; the version check is still missing.

---

## Phase 4: User Story 5 — Missing tools and mismatches stop the run early (Priority: P2)

**Goal**: A missing tool or a driver that differs from the machine's web view stops the run before
anything starts, with a message that names the problem and the remedy (FR-011, SC-006).

**Independent Test**: Hide a tool from `PATH`, and separately use a driver with another version: each
stops the run within 10 seconds with exit status 2, a message naming problem and remedy, nothing started.

### Tests for User Story 5 (write first)

- [x] T033 [P] [US5] Extend `scripts/e2e/lib/preflight.test.ts`: the driver version is read from
      `share/webkit-webdriver/version` next to the `bin/` directory the driver was found in (use the path
      as found on `PATH`, not the symlink target: the delivered driver is a symlink into another
      package); without that file it falls back to the installed package version of `webkit2gtk-driver`
      (an injectable command runner, `dpkg-query -W -f='${Version}' webkit2gtk-driver`), reduced to the
      upstream `major.minor.patch` (`2.44.2-0ubuntu0.24.04.1` becomes `2.44.2`, an epoch prefix is
      dropped); the web view version comes from `pkg-config --modversion webkit2gtk-4.1`; the versions are
      compared as `major.minor.patch`; a version that cannot be determined fails and names the missing
      source; each failure message names the problem and the remedy (dev shell, or the distribution
      package) and, for a mismatch, both versions; the result is a tool check as in data-model.md
      (`tools[]`, `driverVersion`, `webviewVersion`, `result`: `failed` if any tool is missing, a version
      is absent, or the two versions differ) and no command runs beyond those named; each command has a
      5 second limit.

### Implementation for User Story 5

- [x] T034 [US5] `scripts/e2e/lib/preflight.ts`: add `checkPreflight` returning the tool check of
      data-model.md, with the messages above. It reports before anything is started.
- [x] T035 [US5] Wire `checkPreflight` into `scripts/e2e/cli.ts` in place of the stub of T029 and add the
      cases to `scripts/e2e/lib/cli.test.ts`: a failed tool check exits 2 before the sweep and the build,
      starts no process, and prints the messages; a passing check passes the resolved tool paths on in
      `E2E_TOOLS`.
- [x] T036 [US5] Manual trials (SC-006), recorded: run with a `PATH` that lacks `xvfb-run`; then with
      `WebKitWebDriver` resolving to a stub whose version file says another version. Each: exit status 2
      within 10 seconds, the message names the tool or shows both versions and the remedy, and
      `ps` shows nothing started by the run.
- [x] T037 [US5] Checkpoint and Stage 1 pull request (T008 to T037): all checks of T032, commit
      `feat(e2e): check tools and versions before starting`. Open the PR after asking the operator.

**Checkpoint**: Stage 1 is complete: one safe command with an early, clear stop.

---

## Phase 5: User Story 3 — A new scenario is short to write (Priority: P2)

**Goal**: A contributor writes only what is specific to a scenario. The helpers of contracts/helpers.md,
the stand-in provider and the interface hooks exist and are documented (FR-006, FR-007, FR-013 to FR-015).

**Independent Test**: A contributor who has not seen the suite writes "create a vault, unlock it, call
one backend command, check its result" in 40 lines or fewer using only the helpers and the README, and it
runs in the suite (SC-005).

- [ ] T038 [US3] Start branch `016-e2e-stage2-helpers` from the updated `origin/main`. Graphify first:
      `graphify query "stand-in http provider streaming server-sent events"` (research R9 records that the
      Rust `sse_body()` and wiremock fixtures and `scripts/lib` harnesses cannot be extended),
      `graphify query "click and type on a page by selector"`,
      `graphify query "create and unlock an instance flow"`. Note the candidates in the PR description.

### Tests for User Story 3 (write first)

- [ ] T039 [P] [US3] `scripts/e2e/lib/page.test.ts` against `fake-driver.testlib.ts`: `click(hook)` and
      `type(hook, text)` resolve a hook per contracts/test-hooks.md (a value starting with `#`, `.` or `[`
      is a selector, any other value is `[data-testid="<value>"]`), choose the displayed element when
      several match and fail naming the hook if none is displayed by the deadline; `press(hook)` clicks
      inside the page through `setTimeout(…, 0)` so the call returns, records the `press` step with the
      runner's clock, and with `{ times: 2 }` dispatches both clicks in one script tick; `closeWindow`,
      `navigate(url)` and `exec` send the W3C calls; `waitForEnd(deadlineMs)` resolves with the time from
      the call to the end of `appPid` or fails at the deadline and records `process-ended`;
      `markedProcesses()` returns the run's marked processes whose executable is the application;
      `sampleUntilEnd(script, intervalMs)` returns the samples taken until the session ends; `invoke` with
      `expectEnd` resolves `{ ended: true }` for a call that gets no answer (User Story 3 scenario 2).
- [ ] T040 [P] [US3] `scripts/e2e/lib/provider.test.ts`: it listens on `127.0.0.1` only, on a port the
      operating system chose; `GET /v1/models` answers one model `stand-in-model` with `data`, `has_more`,
      `first_id`, `last_id`; any other route is 404; `stream-forever` (defaults `intervalMs` 100, `text`
      `tick `) sends the start events then at least two `content_block_delta` within 500 ms and its
      connection has no `closedAt` until the client drops it, after which `closedAt` is set within
      200 ms; `stream-then-finish` (defaults `chunks` 5, `intervalMs` 200) sends that many deltas, then
      `content_block_stop`, `message_delta`, `message_stop`, and ends; `error` (default `status` 500)
      answers at once with that status and a JSON error body; `behave` applies to later requests only;
      `requests()` records method, path and body and never `x-api-key` or `authorization` (send a
      sentinel and search the serialized records for it); `waitForOpen` resolves; `close` ends open
      streams and frees the port.
- [ ] T041 [P] [US3] `scripts/e2e/lib/flows.test.ts` against the fake driver: `createAndUnlock` issues the
      backend call `create_instance`, clicks the `instance-entry` whose `data-instance-name` is the
      name, types the generated passphrase into `#unlock-passphrase`, clicks `[form="unlock-form"]`
      (never a key press: Enter did not submit the form in the spike) and waits for `/workspace/`;
      `openChat` clicks `open-chat` and waits for `/chat/`; `connectProvider` issues `add_provider`
      (`kind: 'api_key'`, `adapter: 'anthropic'`, the provider's base address, a generated key) then
      `load_model`; `startReply` issues `send_message` and records `reply-streaming` when the provider
      has an open connection.

### Implementation for User Story 3

- [ ] T042 [P] [US3] `scripts/e2e/lib/page.ts`: the interaction helpers of T039.
- [ ] T043 [P] [US3] `scripts/e2e/lib/provider.ts`: the stand-in provider of contracts/stand-in-provider.md,
      including the records with the runner's one clock and `closedAt` taken from the socket's close
      event. It serves nothing else and never outlives its scenario.
- [ ] T044 [US3] `scripts/e2e/lib/flows.ts` (after T042, T043): `createAndUnlock`, `openChat`,
      `connectProvider`, `startReply` as in T041.
- [ ] T045 [US3] In `scripts/e2e/lib/instance.ts` expose the page helpers on the instance
      (`invoke`, `click`, `type`, `press`, `closeWindow`, `navigate`, `waitForEnd`, `markedProcesses`,
      `sampleUntilEnd`) and add `ctx.provider(behavior?)` in `scripts/e2e/lib/scenario.ts`, ended with the
      context.
- [ ] T046 [P] [US3] Hook: in `src/components/onboarding/InstancesList.vue` add
      `data-testid="instance-entry"` and `:data-instance-name="i.name"` to the entry button (2 lines).
- [ ] T047 [P] [US3] Hook: in `src/components/workspace/ChatFab.vue` add `data-testid="open-chat"` to the
      link (1 line).
- [ ] T048 [P] [US3] Hook: in `src/pages/chat/[instance].vue` add `data-testid="lock-instance"` to both lock
      buttons, the one in the sidebar and the one in the header (2 lines, and nothing else in this file).
- [ ] T049 [US3] Verify the hooks: `pnpm check:templates`, `pnpm typecheck`, `pnpm lint`,
      `pnpm format:check`; `wc -l` of the chat page equals the T002 baseline plus 2, the other two files
      plus their stated lines. Behavior and appearance are unchanged (attributes only).
- [ ] T050 [US3] `scripts/e2e/scenarios/create-and-unlock.test.ts`: create and unlock a vault, call one
      backend command (`list_instances`) and check its result, in 40 lines or fewer, using only helpers.
      It is also the model the README shows.
- [ ] T051 [US3] `scripts/e2e/README.md`: how to run (`pnpm test:e2e`, options, `--app`), the tools and
      where they come from, how to write a scenario (copy T050), the helper list of contracts/helpers.md,
      the hook table of contracts/test-hooks.md, the rules for scenarios (hooks only, no key submit, no
      waiting for a call that closes the app, no fixed sleeps, nothing started outside the context, `needs`
      for a relaunching build), how the command and the scenario files talk (the environment of the
      Format section), where the run directory is and what is in it, and troubleshooting for a leftover
      run. Run `pnpm exec prettier --write scripts/e2e/README.md`.
- [ ] T052 [US3] First pass of SC-005, recorded: read only the README and write the scenario T050 again
      from scratch in a scratch file; it must come to 40 lines or fewer. Fix the README where it fell
      short.
- [ ] T053 [US3] Run `pnpm test:e2e` (smoke and create-and-unlock pass). Commit the checkpoint
      `feat(e2e): scenario helpers, stand-in provider and test hooks`.

**Checkpoint**: a scenario can be written from the README and the helpers.

---

## Phase 6: User Story 4 — A failed scenario explains itself (Priority: P2)

**Goal**: For a failed scenario the run keeps what a maintainer needs to see why without running it
again, and says where it is (FR-019, FR-020, SC-004).

**Independent Test**: Seed three failures (the application never starts, the process does not end after
the lock control, the provider connection stays open); for each, the kept material names the step that
failed. The first is tested here, the other two after the close scenarios exist (T078).

### Tests for User Story 4 (write first)

- [ ] T054 [P] [US4] `scripts/e2e/lib/artifacts.test.ts`: on failure the scenario directory under the run
      directory holds `timeline.json` (all steps with `atMs` and an ISO time, the failure message, the
      deadline reached if any), `screenshot.png` taken before teardown (absent, with a note in the
      timeline, if the application already ended), `driver.log` (driver, webview driver and application
      output in arrival order, each line timestamped) and `provider.json` (connections and requests);
      on a pass none of these exist and only `report.json` (and `build.log` if built) remain in the run
      directory; `--keep` keeps them for a pass too; capturing never throws when the session is gone.
- [ ] T055 [P] [US4] Extend `scripts/e2e/lib/report.test.ts`: `failedStep` is the last step reached before
      the failure or the failing wait's description; `material` is the scenario directory; the printed
      line of a passing scenario with a `press` and a `process-ended` step includes "press to process end
      <seconds>"; a failure line names the failed step and the material directory.

### Implementation for User Story 4

- [ ] T056 [US4] `scripts/e2e/lib/artifacts.ts`: `captureFailure` writing the four files above.
- [ ] T057 [US4] Use it: in `scripts/e2e/lib/scenario.ts` call `captureFailure` from the `onFailure`
      extension point of T021 before teardown, for a thrown error and for a reached deadline; in
      `scripts/e2e/lib/report.ts` fill `failedStep`, `material` and the press-to-end text; pass `--keep`
      through `E2E_KEEP` from `scripts/e2e/cli.ts`; remove the directory of a passing scenario unless
      kept.
- [ ] T058 [US4] Seeded failure 1 (SC-004), recorded: point `--app` at an executable script that only
      sleeps and run `pnpm test:e2e --close-behavior exit --grep smoke` with it; the material names the
      step that failed (the instance never became ready), and nothing is left running. Commit
      `feat(e2e): keep failure material and report step times`, then the Stage 2 pull request (T038 to
      T058), after asking the operator.

**Checkpoint**: Stage 2 is complete: helpers, provider, hooks, README and failure material.

---

## Phase 7: User Story 2 — The close promises of spec 013 are checked automatically (Priority: P1) 🎯 MVP

**Goal**: The scenarios done by hand for spec 013 run on their own (FR-016 to FR-018): lock while a
reply streams, window close while a reply streams, the closing page in a light and a dark scheme, lock
twice, and, with a relaunching build, the relaunch.

**Independent Test**: With the stand-in provider streaming without end, unlock a vault, start a reply and
press the lock control: the scenario passes only if the process ends within 4 seconds and the provider
sees its connection close within 1 second of the press. Weaken the behavior on purpose and it fails
(T077).

- [ ] T059 [US2] Start branch `016-e2e-stage3-close` from the updated `origin/main`. Graphify first:
      `graphify query "close promises drain ladder deadlines"` and
      `graphify query "assert a process ended within a deadline"`; note the candidates.
- [ ] T060 [P] [US2] `scripts/e2e/lib/close-promises.test.ts`: the process ends within 4 seconds, the
      provider connection closes within 1 second of the press, and the relaunch within 10 seconds; these
      thresholds stay fixed for conformance. A run with `E2E_TIME_SCALE` other than 1 is explicitly
      non-conformant and reports its scale rather than changing these assertions.
- [ ] T061 [P] [US2] `scripts/e2e/lib/close-promises.ts`: the three numbers, with a comment naming their
      source (spec 013: drain ladder 1 s cooperative, 3 s total, 0.5 s grace).
- [ ] T062 [US2] `scripts/e2e/scenarios/lock-while-streaming.test.ts`: provider `stream-forever`;
      `createAndUnlock`, `openChat`, `connectProvider`, `startReply`; wait until the provider's
      connection has been open for at least 800 ms; `press('lock-instance')`; `waitForEnd` within the
      4 seconds of `close-promises`; the connection's `closedAt` minus the press time is at most 1 second
      (records `provider-closed`); no `[role="alert"]` in any sample from `sampleUntilEnd` and none just
      before the press. The pid tracked is the original `appPid`, so the scenario is correct on a
      relaunching build too.
- [ ] T063 [P] [US2] `scripts/e2e/scenarios/window-close-while-streaming.test.ts`: the same state, then
      `closeWindow()` instead of the press; the same three outcomes.
- [ ] T064 [P] [US2] `scripts/e2e/scenarios/closing-page.test.ts`: for `colorScheme` `light` and `dark`,
      each in its own instance: `navigate('tauri://localhost/closing.html')`; first assert that
      `matchMedia('(prefers-color-scheme: dark)').matches` equals the requested scheme (a scheme that did
      not apply is a clear failure, research R7); then `document.body.textContent.trim()` is empty,
      exactly one `.ring` exists, and the body's background equals a probe element's background set to
      `var(--page)`; finally the two schemes' backgrounds differ.
- [ ] T065 [P] [US2] `scripts/e2e/scenarios/lock-twice.test.ts`: an unlocked vault with nothing running;
      `press('lock-instance', { times: 2 })`; the original pid ends once within 4 seconds; for the next
      5 seconds `markedProcesses()` never holds more than one process; no `[role="alert"]` in the
      samples.
- [ ] T066 [US2] Run the four scenarios with `pnpm test:e2e --grep <name>`; each finishes in under 30
      seconds (SC-007). Record the durations and the `press to process end` times in the "Validation
      record". Commit `feat(e2e): check the vault close promises of spec 013`.

### The relaunch scenario (needs a release build; two checks first)

- [ ] T067 [US2] Check A, recorded: build a release with `pnpm tauri build --no-bundle` and run
      `pnpm test:e2e --app <its path> --grep smoke`. If `tauri-driver` cannot drive a release-profile
      binary, stop here: record the failure and report it to the operator; T068 to T073 wait for a
      decision, and T062 to T066 are unaffected.
- [ ] T068 [US2] Check B, recorded, with a throwaway script that is not committed: with the release
      binary, press the lock control and observe from outside (a) the original pid is gone, (b) a new
      marked process of the same binary appears, so the marker is inherited by a relaunch (research R8),
      (c) whether the virtual screen started with `-fbdir <dir>` keeps a current XWD image that Node can
      read, and what a blank screen and a painted window look like in it. Record the values that the
      painted check of T070 will rely on. If (c) fails, keep the scenario to steps 1, 2 and 4 of research
      R11 and report the difference from FR-018 to the operator; do not drop it silently.
- [ ] T069 [P] [US2] `scripts/e2e/lib/framebuffer.test.ts` with a synthetic XWD buffer built in the test:
      the header is parsed (width, height, bytes per line, bits per pixel, header size, colormap
      entries), a blank image is reported as not painted and one with a window-sized non-blank region as
      painted, and a truncated file is an error, not a pass.
- [ ] T070 [P] [US2] `scripts/e2e/lib/framebuffer.ts`: `readFramebuffer` and `isPainted` per T069 and the
      values recorded in T068.
- [ ] T071 [US2] In `scripts/e2e/lib/instance.ts` add the option `framebufferDir` that adds
      `-fbdir <dir>` to the virtual screen's arguments in `driverCommand`, and a case for it in
      `scripts/e2e/lib/instance.test.ts`.
- [ ] T072 [US2] `scripts/e2e/scenarios/relaunch-after-lock.test.ts` declaring that it needs the
      `relaunch` close behavior: create and unlock, press the lock control; (1) the original pid is gone within 4
      seconds; (2) a new marked process of the same binary with another pid exists within 10 seconds
      (records `relaunch-seen`); (3) the screen shows a painted window (T070), if T068 allowed it;
      (4) stop that process, start a fresh instance with `reusesRoot` over the same data and check that
      `instance-entry` for the created name is displayed and `location.pathname` is `/`, which is what
      the relaunched window shows. If the application does not relaunch, the failure message names that
      the release binary did not relaunch.
- [ ] T073 [US2] Run the relaunch scenario against the release build, record the result and time, and
      against the debug build, where it must be reported as skipped with its reason. Commit
      `feat(e2e): check the relaunch after a lock` and open the Stage 3 pull request (T059 to T073) after
      asking the operator.

**Checkpoint**: the spec 013 close promises run on their own; the relaunch runs with a release build.

---

## Phase 8: Stage 4 — Proof of the success criteria (records only, no product code)

**Purpose**: Show that the suite does what the spec promises, with the recipes of quickstart.md. Outcomes
go to the "Validation record". Branch `016-e2e-stage4-validation`; a pull request with the record.

- [ ] T074 SC-001 and SC-007: `pnpm test:e2e --app <a debug build already built>`; the close scenarios
      each finish in under 30 seconds and the run in under 5 minutes; the relaunch scenario is listed as
      skipped with its reason; exit status 0; with the release build by path all five run.
- [ ] T075 SC-002: 20 consecutive runs, 10 of them with `kill -9` on the runner at a random moment
      (`sleep $((RANDOM % 25))` before the kill), with your own Holzi running throughout. After each run:
      no marked process remains once the next run's sweep has run, no window appeared, your Holzi still
      runs and its data listing is unchanged, and every run after a killed one starts normally.
- [ ] T076 Confirm the isolation of FR-006 on a real run: while a scenario runs, `ls` the maintainer's
      data, config, cache and runtime directories and home for changes made by it (none), and confirm
      no `xdg-desktop-portal` process was started by the run.
- [ ] T077 SC-003, in a scratch change reverted after each trial: (a) do not cancel the stream when the
      vault closes, expect `lock-while-streaming` and `window-close-while-streaming` to fail; (b) do not
      replace the page by the closing page, expect `closing-page` to fail; (c) make the process not end,
      expect `lock-while-streaming` and `lock-twice` to fail; (d) make a release build not relaunch,
      expect `relaunch-after-lock` to fail. Restore each and expect a pass. Record every trial.
- [ ] T078 SC-004: seeded failures 2 (the process does not end after the lock control, from T077 (c)) and
      3 (the provider connection stays open, from T077 (a)), with failure 1 from T058. For each, the
      material names the failed step, so no second run is needed. Record.
- [ ] T079 SC-005, final pass: ask the operator for a contributor who has not seen the suite, or start a
      fresh session, give it only `scripts/e2e/README.md` and ask for "create and unlock a vault, call one
      backend command, check its result". It must take 40 lines or fewer and run. Record what tripped it
      and fix the README.
- [ ] T080 Commit the record as `docs(specs): record the validation of the e2e suite`; open the Stage 4
      pull request after asking the operator.

---

## Phase 9: User Story 6 — The same suite runs in the repository's CI (Priority: P3)

**Goal**: The suite runs on `ubuntu-24.04` with the tools from the distribution; a failing run publishes
its material (FR-021). Separable: nothing above waits for it.

**Independent Test**: A CI run on a clean runner passes the same scenarios as the ordinary local run; a
deliberately failing scenario fails the job and uploads the run directory.

- [ ] T081 [US6] Branch `016-e2e-stage5-ci` from the updated `origin/main`. In `.github/workflows/ci.yml`
      add a job `e2e` on `ubuntu-24.04` with `permissions: contents: read` and a `timeout-minutes`: check
      out (same pinned action and `persist-credentials: false` as the other jobs), Node 22.19.0 through
      the same pinned action, `corepack pnpm install --frozen-lockfile --ignore-scripts`, the apt
      packages of the Rust job plus `webkit2gtk-driver` and `xvfb`, the Rust toolchain and cache steps as
      the Rust job uses them (shared key for the default features), `tauri-driver` installed with
      `cargo install tauri-driver --locked --version 2.0.6` and cached, then `corepack pnpm test:e2e`. On
      failure upload `src-tauri/target/e2e` as an artifact. Every third-party action is pinned by full
      commit hash like the existing ones: look up the current release commit of each new action and
      record the tag in a comment. Outside the Nix shell the bridge script runs the command unchanged.
- [ ] T082 [US6] The job relies on the package fallback of the version check (T033): confirm on the runner
      that the driver's and the web view's package versions are equal and the preflight passes.
- [ ] T083 [US6] Verify on a pushed branch, recorded with the run links: a clean run passes with the
      relaunch scenario skipped; a scratch commit that breaks one scenario makes the job fail and the
      artifact downloadable. Revert the scratch commit. Commit `ci: run the e2e suite on ubuntu` and
      open the Stage 5 pull request after asking the operator.

---

## Phase 10: Polish and cross-cutting

- [ ] T084 Full CI parity from a clean state: `pnpm typecheck`, `pnpm typecheck:scripts`, `pnpm lint`,
      `pnpm format:check`, `pnpm check:chat-state`, `pnpm check:vault-lifecycle`, `pnpm check:templates`,
      `pnpm check:e2e-lib`, `python3 scripts/ci/check-docs.py`, `git diff --check`.
- [ ] T085 Compare `wc -l` with the T002 baseline: the chat page grew by exactly 2 lines, every new file
      is under 500 lines. Record the numbers in the Baseline section.
- [ ] T086 [P] Bring `plan.md`, `research.md`, `data-model.md` and `contracts/` in line with what was
      built: the files the plan did not list (`page.ts`, `flows.ts`, `artifacts.ts`, `framebuffer.ts`,
      `fake-driver.testlib.ts`, and `session.ts` if it was split), the outcomes of T067 and T068, and any
      renamed helper. Run `pnpm exec prettier --write specs/016-e2e-testing` twice and confirm it is
      stable.
- [ ] T087 [P] In `specs/013-vault-lifecycle-isolation/tasks.md` add a note to T050 that its automatable
      part (quickstart scenarios 1 and 2, the closing page and the relaunch) is covered by spec 016 with
      the scenario names, and that T048 (relaunch under `tauri dev`) and T049 (local inference stop
      time) stay manual.
- [ ] T088 Traceability: for FR-001 to FR-021 and SC-001 to SC-007 write down in the Validation record the
      test or the recorded result that covers each. A gap is closed or reported.
- [ ] T089 Run `/speckit-analyze` (the constitution requires it to check plans against the constitution)
      and fix findings. Fold in the four spec alignment items of the plan (FR-006, FR-018, the closing
      page scenario, "no error appears") as a small reviewed spec change or record why not.
- [ ] T090 With the operator's agreement, remove the throwaway spike: the worktree
      `.worktrees/016-e2e-spike` and the local branch `spike/e2e-rig`, which the suite replaces, and the
      old local branch `spike/vault-gateway` (task T051 of spec 013).
- [ ] T091 Open any remaining pull requests in the structure below after asking the operator. Merge by
      rebase or merge commit, never squash; merge-commit subjects use a Conventional Commits header.

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1 (Stage 0)** has no dependency and unblocks everything that runs the tools; Phase 2 can be
  written in parallel, but nothing that starts the application can run before T006 passes.
- **Phase 2** blocks every story. Tests T010 to T015 first, then T016 to T019 in parallel, then T020, then
  T021.
- **Phase 3 (US1)** after Phase 2. **Phase 4 (US5)** after Phase 3 (it replaces the stub in `cli.ts`).
  Stage 1 ends with T037.
- **Phase 5 (US3)** after Stage 1. **Phase 6 (US4)** after Phase 5 (it uses the fake driver, the scenario
  wrapper and the report). Stage 2 ends with T058.
- **Phase 7 (US2)** after Stage 2: the scenarios need the provider, the flows, the hooks and the page
  helpers. T067 and T068 gate T069 to T073; T062 to T066 do not depend on them.
- **Phase 8** after Stage 3. **Phase 9 (US6)** after Stage 1 and is separable; it can run in parallel with
  Stages 2 and 3, but its verification (T083) is more useful once scenarios exist.
- **Phase 10** at the end.

### Story dependencies

- US1: after the foundation, no other story. US5: extends US1's command. US3: after US1. US4: after US3.
  US2: after US3 and US4 (needs the helpers and the failure material to be diagnosable). US6: after US1.

### Within a stage

- Tests first and expected to fail; implementation next; then the stage's manual verification and the
  commit. One commit per checkpoint, Conventional Commits, no model trailer.

## Parallel Example

```text
# Foundation tests together (different files):
T010 processes.test.ts   T011 ports.test.ts   T012 webdriver.test.ts + fake-driver.testlib.ts
T013 instance.test.ts    T014 scenario.test.ts   T015 preflight.test.ts

# Then the independent implementations together:
T016 processes.ts   T017 ports.ts   T018 webdriver.ts   T019 preflight.ts

# Stage 2 helpers together:
T039 page.test.ts   T040 provider.test.ts   T041 flows.test.ts
T042 page.ts        T043 provider.ts        T046 / T047 / T048 the three hooks

# Stage 3 scenarios together (different files):
T063 window-close-while-streaming   T064 closing-page   T065 lock-twice
```

## Implementation Strategy

### MVP first

1. Stage 0 (tools), then Stage 1 (the command, US1 and US5): a safe, checked command with a smoke scenario.
2. Stage 2 (US3 and US4) and Stage 3 (US2): the helpers, the diagnostics and the close scenarios. With
   these the two P1 stories are complete and the spec 013 promises are checked automatically.
3. **Stop and validate** with Stage 4 before adding CI.

### Incremental delivery

1. Each stage is a pull request that leaves the suite usable: after Stage 1 it runs and stops early and
   clearly; after Stage 2 a scenario is short to write and a failure explains itself; after Stage 3 the
   close promises are covered; Stage 5 adds the CI job without changing the local flow.
2. The relaunch scenario (T067 to T073) may land in a later pull request than the other four close
   scenarios if its two checks take time.

## Notes

- [P] tasks touch different files and have no dependency on an unchecked task.
- A task that says "recorded" writes its outcome to the "Validation record" below before it is ticked.
- If T067 or T068 fail, the spec alignment item on FR-018 is reported to the operator, not decided here.

## Baseline

Recorded at T002 on 2026-09-22, branch `016-e2e-stage0-tools` from `origin/main` at `8e0b00b`, before any edit.

| Check                                                      | Result                   |
| ---------------------------------------------------------- | ------------------------ |
| `pnpm typecheck:scripts`, `pnpm lint`, `pnpm format:check` | pass                     |
| `pnpm check:chat-state`                                    | 45 tests, 45 pass        |
| `pnpm check:vault-lifecycle`                               | 5 tests, 5 pass          |
| `pnpm check:templates`                                     | 33 Vue templates compile |

| File                                          | Lines |
| --------------------------------------------- | ----- |
| `src/pages/chat/[instance].vue`               | 1316  |
| `src/components/onboarding/InstancesList.vue` | 44    |
| `src/components/workspace/ChatFab.vue`        | 20    |

_T085 adds the closing numbers here._

## Validation record

### Stage 0 (T003 to T006), 2026-09-22

- **T003** The atoms range `1a292194…..d5c48d0e…` holds two commits: `480a0e37` (the tooling change) and
  `d5c48d0e` (the merge of haexmas/atoms#32). Nothing else arrives with the new pin.
- **T004** The `revision` in `.spaex/manifest.json` is now the full hash
  `d5c48d0eb6662da84086bf69a3ca041a494e2b26`. `spaex install` first stopped with
  `pinned-revision-not-found`; `git fetch origin` in spaex's own publisher clone under
  `~/.local/share/spaex/repos/` fixed that. The installed run used
  `spaex install --speckit-agents claude,codex --no-install-hooks`: the Spec Kit selection is what the
  lock recorded before (a bare `spaex install` refuses to run without one), and the hooks were skipped
  because the graphify hook refuses any branch that is not `main`, which would only have recorded a
  branch-specific failure. `install.lock` therefore keeps its earlier hook state.
- **T005** Changed by the install: `flake.nix` (reads `.devshell/packages.nix`), the new
  `.devshell/packages.nix`, `.spaex/generated/nix-packages.json` (gains `xvfb-run`),
  `.spaex/manifest.json` and `.spaex/install.lock` (tracked despite its ignore rule: new revisions, the
  new file's hash, a new generation id and the Spec Kit declaration fingerprint). No other tracked file
  changed. No absolute path in any delivered file. `git ls-files .devshell/packages.nix` lists it.
- **T006** In the shell, through the host bridge:

  ```text
  tauri-driver     /nix/store/…-tauri-driver-2.0.6/bin/tauri-driver
  WebKitWebDriver  /nix/store/…-webkit-webdriver-2.52.6/bin/WebKitWebDriver
  xvfb-run         /nix/store/…-xvfb-run-1+g87f6705/bin/xvfb-run
  driver version file:                       2.52.6
  pkg-config --modversion webkit2gtk-4.1:    2.52.6
  ```

  `tauri-driver --help` lists `--port`, `--native-port` and `--native-driver`, the options the suite
  passes.

### Stage 1 (T008 to T037), 2026-09-22

- **T008** Graphify, from the primary checkout: the candidates for spawning a process were the Rust
  `process.rs` (`cli_delegate`) and its `spawn`, for reading process state the same file, for a wire
  protocol client the Rust adapters, and for waiting `waitForTurnTerminal` in the chat composable. All are
  Rust or frontend code in other processes; none can serve a Node runner, so nothing was extended.
- **T031** Partial validation only: the smoke scenario passed against a debug binary of the spike
  (`--app`) and against the debug build the command made itself in this worktree (`pnpm test:e2e --grep
smoke`, built from a reflink copy of an earlier build cache): `passed smoke-start`, 6.1 s, exit 0.
  The application environment was read from `/proc`: `DISPLAY=:99` (the virtual screen),
  `GDK_BACKEND=x11`, no `WAYLAND_DISPLAY`, `HOME`, `XDG_DATA_HOME` and `XDG_RUNTIME_DIR` inside the
  instance root, `DBUS_SESSION_BUS_ADDRESS=disabled:` and the marker. A checksum listing of the
  maintainer's data directory is identical before and after. No maintainer Holzi was running during
  this check, so the required concurrent-instance isolation case and the visual no-window check remain
  unverified. T031 stays open until that case is run.
  - Ctrl-C (SIGINT) at about 4 s: exit status 130, no process with a run marker left.
  - `kill -9` on the runner at about 4 s: 9 marked processes stayed behind; the next run reported
    "removed 9 leftover process(es) of an earlier killed run", passed, and left none.
- **T035** The cases the task lists already exist in `scripts/e2e/lib/cli.test.ts`: a failed tool check
  exits 2 before the sweep and the build and starts nothing, and a passing one hands the tool paths on
  in `E2E_TOOLS`.
- **T036** Missing `xvfb-run` (its directory removed from `PATH`): exit status 2 after 844 ms, message
  names the tool and the remedy, no marked process. A driver whose version file says 2.44.2 against the
  web view's 2.52.6: exit status 2 after 820 ms, message shows both versions and the remedy, no marked
  process.
- **T032 and T037** One commit covers both checkpoints, because the command imports the preflight it was
  going to stub. The environment of the application also pins `GDK_BACKEND=x11` and drops the display
  variables of the parent; `data-model.md` and `research.md` R5 say so.

_Filled by later tasks: T052, T058, T066, T067, T068, T073 to T083 and T088._
