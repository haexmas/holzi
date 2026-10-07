---
description: 'Task list for spec 043-android-build'
---

# Tasks: holzi für Android

**Input**: Design documents from `/specs/043-android-build/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/platform-capabilities.md](./contracts/platform-capabilities.md), [contracts/picked-file.md](./contracts/picked-file.md), [contracts/import-instance.md](./contracts/import-instance.md), [contracts/android-plugin.md](./contracts/android-plugin.md), [contracts/e2e-android.md](./contracts/e2e-android.md), [contracts/ci.md](./contracts/ci.md), [quickstart.md](./quickstart.md)

**Tests**: Included. The spec requires the desktop e2e suite on an Android emulator on every PR (FR-033a–c) plus Rust unit tests in sibling `*_tests.rs` files, pure TypeScript checks under `scripts/check-*.ts` and `scripts/e2e/lib/**/*.test.ts`. Tests come first inside each story and fail before the code exists. Real gestures, the system file picker, notifications, the camera cutout and network switches are manual on a real phone (quickstart §3, §5–§8).

**Organization**: By delivery stage of the plan (each stage is one PR and ships on its own): Stage 1a = Phase 2 (foundational: builds and starts), Stage 1b = Phase 3 (US2: e2e on Android), Stage 1c = Phase 4 (US1), Stage 1d = Phase 5 (US2: release), Stage 2 = Phase 6 (US3), Stage 3 = Phase 7 (US4), Stage 4 = Phase 8 (US5), Stage 5 = Phase 9 (US6). Priorities: P1 US1, US2; P2 US3; P3 US4; P4 US5; P5 US6. The branch `043-android-build` starts from `main` after PR #308 (Android devShell).

**Shipping note**: From Stage 1b on, the Android run is a required check. Scenarios that only a later stage makes pass are listed as `pending` with that stage in `scripts/e2e/platform-exclusions.ts`; each stage removes its own entries; after Stage 5 `pending` is empty (SC-007). No migration in any stage.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: User story from spec.md (US1–US6)

**Conventions for every task**: files stay ≤ 500 lines (new code goes into new files rather than growing large ones); Rust tests in sibling `*_tests.rs`; no localized text in Rust; no secret or absolute local path in a log, error, event or committed file; Kotlin only in `src-tauri/plugins/holzi-android/android/` (never in `src-tauri/gen/android` except the `configChanges` and signing lines named below); commits follow Conventional Commits without agent attribution; Rust runs through `nix develop --command scripts/with-nix-host-bridge.sh` with `CARGO_BUILD_JOBS=4` and `-j 4`, one test target at a time; Android builds run in `nix develop` (`pnpm tauri android …`); after every Rust type change run `pnpm generate:ts-types` and commit `src/types/bindings/`; `pnpm format` before each commit; every `cfg(target_os = "android")` branch also compiles under `cargo check --target aarch64-linux-android` (CI job `android-build` runs it with clippy).

---

## Phase 1: Setup

- [x] T001 Worktree `.worktrees/043-stage-1a` on branch `043-stage-1a` from `main` after PR #315 (spec documents; the docs branch was `043-android-build`), which contains PR #308; real `pnpm install --frozen-lockfile` inside `nix develop`; confirm `nix develop --command sh -c 'echo $ANDROID_HOME $NDK_HOME $JAVA_HOME; rustc --print sysroot'` shows the Android SDK, NDK 28.2.13676358, JDK 17 and the rust-overlay toolchain
- [x] T002 [P] Before the first new name, run `graphify query` for: "close policy exit restart", "file dialog path attachment import", "instance create pending marker rollback", "device preference scope", "delegate availability model inventory", "sync reconnect after connection loss", "hostname device alias", "model download size catalog recommend", "e2e device host platform", and write the reused names (and any that already exist under another name) as a short note at the end of this task; adjust file names in later tasks only if graphify shows a clash (done 2026-10-07: two clashes. `extensions/bridge/methods.rs` has a private `platform()` that reports the OS name to extensions (`ApplicationContext.platform`); it reads the new capability table instead (one rule, one place; T012). The frontend already has `Platform = 'mac' | 'default'` for key chords in `src/lib/wm/keybindings.ts` (`detectPlatform`), so the composable of T035 is named `useDeviceCapabilities.ts`, not `usePlatform.ts`; tasks T047, T049 and T051 read it under that name. Reused, not duplicated: `vault_gate::close_policy` and `instances/close_effects.rs` (T013), `sync::reconnect_missing` in `src-tauri/src/sync/mod.rs:42` as the resume entry point (T058), `hardware/hostname.rs` (T060), `stt/catalog.rs` `recommend_tiers` and `catalog/mod.rs` `CATALOG` (T074, T075), `PrefScope::Device` preferences (T048), `scripts/e2e/lib/device.ts` `Device` and `group.ts` (T020, T024); no other new name exists under another name)

---

## Phase 2: Foundational — Stage 1a „baut und startet“ (blocks all stories)

**Purpose**: holzi builds for Android, starts on an emulator and a phone, can create, close and unlock a vault; CI builds the APK on every PR.

- [x] T003 Write `docs/adr/0010-android-platform.md` (format of `0009-…`): same Rust core and UI on Android; one local plugin crate for platform code; committed `src-tauri/gen/android`; certificate check through `rustls-platform-verifier` with the lettre patch; closing the vault ends the app (ADR 0003 unchanged); local models on Android use the desktop loader (mistralrs, GGUF, CPU) — this **replaces** the "native MLC backend" part of `0002-local-model-profiles.md` (add a "Superseded in part by ADR 0010" line there); measurement of SC-009 is added in T089. Cross-refer from research R17
- [x] T004 Run `nix develop --command pnpm tauri android init --ci` and commit `src-tauri/gen/android`; change `.gitignore` line `src-tauri/gen/` to `src-tauri/gen/schemas/`; keep the generator's own `.gitignore` files; in `gen/android/app/src/main/AndroidManifest.xml` extend `android:configChanges` by `density|fontScale|layoutDirection` (research R1, FR-009); grep the committed tree for `/home/`, `/nix/store/`, `local.properties` and abort if any absolute path is committed (constitution II)
- [x] T005 [P] `src-tauri/tauri.conf.json`: `bundle.android.minSdkVersion` 24 (template default, stated explicitly; changed to 26 during T017: cpal links `libaaudio`, which the NDK has from API 26 on, so the link step failed with 24; also `minSdk = 26` in `gen/android/app/build.gradle.kts` and the plugin module, and `aarch64-linux-android26-clang` in CI), version-derived `versionCode` (Tauri default, research R15); generate the Android launcher icons from the existing icon with `pnpm tauri icon` and commit only the Android outputs
- [x] T006 Create the plugin crate `src-tauri/plugins/holzi-android/` (crate `tauri-plugin-holzi-android`): `Cargo.toml`, `build.rs` with `tauri_plugin::Builder::new(COMMANDS).android_path("android")`, `src/lib.rs` (`init()`, `HolziAndroidExt`), `src/desktop.rs` (every method a no-op / `None`), `src/mobile.rs` (`register_android_plugin("space.haex.holzi.android", "HolziAndroidPlugin")`), `android/` Gradle module (`build.gradle.kts`, `src/main/AndroidManifest.xml`, `src/main/java/space/haex/holzi/android/HolziAndroidPlugin.kt` with an empty `@TauriPlugin` class); add it as a path dependency in `src-tauri/Cargo.toml` and register `.plugin(tauri_plugin_holzi_android::init())` in `src-tauri/src/lib.rs` (contract android-plugin.md) (done: the switch is `target_os = "android"`, not `cfg(mobile)` — iOS is out of scope and behaves like the desktop; files `src/android.rs` and `src/other.rs`; Kotlin package `space.haex.holzi.android`)
- [x] T007 In `src-tauri/plugins/holzi-android/android/build.gradle.kts`: add the Maven repository `https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/` and `implementation("org.rustls:rustls-platform-verifier:<version>")` with the version of `rustls-platform-verifier-android` from `src-tauri/Cargo.lock`; add `consumer-rules.pro` with `-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }` as `consumerProguardFiles` (research R2) (done: the app resolves the plugin's dependencies with its own repositories, so the Maven repository is added through `rootProject.allprojects`)
- [x] T008 Tests first, `src-tauri/src/tls/mod_tests.rs`: `init_platform_verifier()` is a no-op that returns `Ok` on desktop and is idempotent. Then `src-tauri/src/tls/mod.rs` and `src-tauri/src/tls/android.rs`: on Android take `ndk_context::android_context()`, attach the current thread to the `JavaVM` (`jni` 0.22) and call `rustls_platform_verifier::android::init_with_env(env, context)` once (`OnceLock`); add `jni = "0.22"` and `ndk-context = "0.1"` under `[target.'cfg(target_os = "android")'.dependencies]` in `src-tauri/Cargo.toml`; call it in `lib.rs` `setup` before any network service starts (research R2)
- [x] T009 Fork `lettre/lettre` to `haexmas/lettre`, branch `fix/android-platform-verifier` from tag `v0.11.23`: in `src/transport/smtp/client/tls.rs` (≈ l. 540–551) use `rustls_platform_verifier::Verifier::new(provider)` when there are no extra roots and always on Android, like `reqwest` 0.13.5 (`async_impl/client.rs:758-780`); add a test for the no-extra-roots path; open the upstream PR (no agent attribution); in `src-tauri/Cargo.toml` add `[patch.crates-io] lettre = { git = "https://github.com/haexmas/lettre", rev = "<full SHA>" }` with a comment pointing to the upstream PR (research R2; constitution IV) (done: haexmas/lettre @ `993d98c5330ce5a3238e2f7fc37e9c9cf4a3f1c1`; with extra roots on Android lettre now returns a TLS error instead of ignoring them; upstream PR lettre/lettre#1167 from branch `android-platform-verifier` on their `master`)
- [x] T010 [P] Compile fixes: `src-tauri/src/extensions/fs/dialogs.rs` — the `blocking_pick_folder` path behind `#[cfg(desktop)]`, on mobile answer `not_available`; `src-tauri/src/extensions/fs/mod.rs` — `desktop_dir()` behind `#[cfg(desktop)]`; confirm `cargo check --target aarch64-linux-android` (default and `--no-default-features`) passes with T009 (done: `cargo clippy --target aarch64-linux-android -- -D warnings` clean with default and `--no-default-features`)
- [x] T011 Tests first, `src-tauri/src/platform/mod_tests.rs`: desktop table values as in data-model.md; a test that pins the Android table as a constant (`ANDROID_CAPABILITIES`) so it is checked on desktop CI too. Then `src-tauri/src/platform/mod.rs` (`PlatformCapabilities` with ts-rs export, `capabilities()` per `cfg`) and `src-tauri/src/platform/commands.rs` (`platform_capabilities`); register the command in `lib.rs`; `pnpm generate:ts-types` (contract platform-capabilities.md) (done: `platform_capabilities` is on the app-scoped allow-list of `vault_gate/invoke.rs`, the vault picker needs it; `extensions/bridge/methods.rs` `platform()` moved here as `os_name()`)
- [x] T012 Route the existing desktop-only checks through `platform::capabilities()`: `src-tauri/src/extensions/shell/mod.rs` (`desktop_only()`), `src-tauri/src/extensions/fs/mod.rs` (`free_paths`), `src-tauri/src/extensions/fs/watch.rs`, `src-tauri/src/hardware/mod.rs` (GPU detection only with `gpuDetection`); add one test per reader that `false` yields "not available" (sibling `*_tests.rs`) (done: one helper `BridgeError::unless(available)` with its test in `extensions/error_tests.rs` instead of one test per reader; the readers pass the field of the table)
- [x] T013 Tests first, `src-tauri/src/vault_gate/mod_tests.rs`: `close_policy()` is `Exit` on Android in debug and release (test against an injected platform flag). Then `src-tauri/src/vault_gate/mod.rs` (`close_policy`), `src-tauri/src/instances/close_effects.rs` (`request_end` → `app.exit(0)`, `force_end` → `std::process::exit` on Android; `tauri::process::restart` never reached on Android) and `src-tauri/src/lib.rs` (`RunEvent::Exit` on Android → `std::process::exit(0)` after the close path ran) (research R4, FR-006) (done: `close_policy_for(capabilities)` is pure and tested in `vault_gate/gate_tests.rs`; the table field `relaunch_on_close` carries the debug/release rule)
- [x] T014 [P] Dev mode (research R13, FR-034): `nuxt.config.ts` — `devServer.host` from `process.env.TAURI_DEV_HOST || 'localhost'`, drop the `TAURI_ENV_HOST` branch and the Vite-era HMR port 1421; `src-tauri/tauri.conf.json` `devCsp` — allow the dev host only in dev; `package.json` scripts `tauri:android:dev` (`tauri android dev`) and `tauri:android:build` (`tauri android build`)
- [x] T015 [P] Tests first, then `scripts/check-android-versions.ts` (`node:test`): parse `.devshell/packages.nix` (`platformVersions`, `buildToolsVersions`, `ndkVersion`), `.devshell/rust-toolchain.toml` (`channel`, `targets`) and the `sdkmanager`/toolchain lines of `.github/workflows/ci.yml` and `.github/workflows/android-release.yml` (when present); fail on any mismatch; `package.json` script `check:android-versions`
- [x] T016 CI job `android-build` in `.github/workflows/ci.yml` (contract ci.md): `actions/setup-java` 17, `sdkmanager` with the pinned packages, `dtolnay/rust-toolchain` 1.98.1 with `aarch64-linux-android` and `x86_64-linux-android`, `Swatinem/rust-cache` key `android`, `gradle/actions/setup-gradle`, `pnpm tauri android build --debug --apk --target x86_64 --target aarch64`, `cargo clippy --target aarch64-linux-android -- -D warnings` (both feature sets), upload artifact `holzi-android-debug`; add `pnpm check:android-versions` to the `documentation` job (done: two jobs, `android-build` (APK, artifact) and `android-lint` (clippy, matrix default/no-default), so they run in parallel; actions pinned by commit like the others)
- [ ] T017 Stage 1a checkpoint: quickstart §1 and §2 on an emulator **and** a real phone (APK from the CI artifact); record in research.md R4 whether `RunEvent::Exit` arrives after `finishAffinity` (else add `onDestroy` + `isFinishing` → `Process.killProcess` in `HolziAndroidPlugin.kt` and say so), and the first-run timings for SC-001/SC-002; PR „feat(android): build and start holzi on Android“

**Checkpoint**: APK from CI installs and starts; vault create → close (app ends) → unlock works.

---

## Phase 3: User Story 2 - APK aus der CI bekommen, getestet im Emulator (P1) — Stage 1b

**Goal**: The desktop e2e suite runs on an Android emulator on every PR as a required check.

**Independent Test**: A PR shows the APK artifact and a green `android` check; breaking an Android-only path turns it red.

- [ ] T018 [US2] Tests first: `scripts/e2e/lib/platform/android.test.ts` with a fake `adb` runner — command sequences for `newData`, `start`, `stop`, `kill`, `alive`, `dispose`, `copyVaultFile`, `keep`; URL rewriting `tauri://localhost` → `http://tauri.localhost`, `holzi-ext://localhost` → `http://holzi-ext.localhost`; chromedriver session capabilities with `androidUseRunningApp: true`; `scripts/e2e/lib/exclusions.test.ts` — rules of data-model.md (reason/counterCase for `excluded`, stage for `pending`, unknown scenario, coverage < 80 %, `pending` not empty when the switch is set)
- [ ] T019 [US2] `scripts/e2e/lib/webdriver.ts`: `newSession(capabilities)` per platform (Linux keeps `tauri:options`); `closeWindow` delegates to the platform
- [ ] T020 [US2] `scripts/e2e/lib/platform/adb.ts` (adb wrapper with synchronous `pidof`, `run-as` tar in/out, `reverse`, `wm size/density`, `cmd uimode`) and `scripts/e2e/lib/platform/android.ts` implementing `DeviceHost` exactly as contract e2e-android.md; chromedriver process launcher in `scripts/e2e/lib/platform/chromedriver.ts`
- [ ] T021 [US2] `scripts/e2e/lib/scenario.ts` and `scripts/e2e/lib/instance.ts`: `ctx.startInstance()` goes through `DeviceHost` (Linux behaviour unchanged); keep `markedProcesses`/`pid`/`root` available through the platform (Android: `pidof` count)
- [ ] T022 [P] [US2] `scripts/e2e/lib/page.ts` (`navigate` URL rewriting) and a platform `stageFile(hostPath) → devicePath` used by `scripts/e2e/lib/extensions.ts` (`install`), `scripts/e2e/lib/appearance.ts` (`importExport`) and `scripts/e2e/lib/passwords.ts` (`addAttachments`) (contract picked-file.md, test seam)
- [ ] T023 [P] [US2] Services for the emulator: `adb reverse tcp:P tcp:P` for every port opened by `scripts/e2e/lib/nostr-relay.ts`, `provider.ts`, `rustfs.ts`, `extension-dev.ts` and local HTTP servers (one helper in `scripts/e2e/lib/ports.ts`); new `scripts/e2e/lib/iroh-relay.ts` starting `iroh-relay --dev` on 3340; Android runs set the iroh relays to `http://127.0.0.1:3340` in `scripts/e2e/lib/sync-flows.ts`/`device.ts`, Linux runs keep the closed port
- [ ] T024 [US2] Mixed groups in `scripts/e2e/lib/group.ts`: device `phone` → Android host, others → Linux host; without `phone` the first device; at most one Android device per group (contract e2e-android.md)
- [ ] T025 [P] [US2] Android mappings in `scripts/e2e/lib/platform/android.ts`: colour scheme via `cmd uimode night yes|no` (for `closing-page`, `settings-color-scheme`), window close = remove the task and check the process ends within the deadline (`window-close-while-streaming`), desktop-sized display `wm size 2560x1600` + `wm density 320` on start, reset on dispose
- [ ] T026 [US2] `scripts/e2e/lib/preflight.ts`: for `--platform android` check `adb` with one device (or `ANDROID_SERIAL`), installed debuggable `com.haex.holzi`, chromedriver major = WebView major (`dumpsys package com.google.android.webview`), `iroh-relay`; name the missing piece on failure
- [ ] T027 [US2] `scripts/e2e/cli.ts`: `--platform linux|android` and `--shard i/n` (by recorded duration from `artifacts/e2e-durations.json`, written by `scripts/e2e/lib/report.ts`; fallback alphabetical round robin); extend `scripts/e2e/lib/cli.test.ts`
- [ ] T028 [US2] `scripts/e2e/platform-exclusions.ts` with the three `excluded` entries of contract e2e-android.md and one `pending` entry (with stage) for every scenario that does not yet pass on the emulator after T019–T027; `scripts/check-e2e-exclusions.ts` + `package.json` script `check:e2e-exclusions` (prints the coverage); run it in the `documentation` CI job
- [ ] T029 [US2] CI: `scripts/ci/android-e2e.sh` (one script for the emulator action: install APK, start chromedriver, run `pnpm test:e2e --platform android --shard $SHARD/3`) and jobs `android-e2e` (matrix shard 1–3, KVM udev rule, `reactivecircus/android-emulator-runner@v2` API 35 `google_apis` x86_64 with AVD snapshot cache, APK from `android-build`, Linux app with warm cache for mixed groups, failure material as artifact) and `android` (`needs: [android-build, android-e2e]`) in `.github/workflows/ci.yml` (contract ci.md)
- [ ] T030 [P] [US2] Update `scripts/e2e/PLATFORMS.md` (Android implemented: driver, lifecycle, services, mixed groups, limits) and `scripts/e2e/README.md` (how to run on an emulator, quickstart §4); note in `specs/033-multi-device-e2e/spec.md` FR-022 that the Android platform document now exists
- [ ] T031 [US2] Stage 1b checkpoint: quickstart §4 locally and the CI run green; tell the operator to add `android` as a required check on `main` (contract ci.md); PR „test(e2e): run the e2e suite on an Android emulator“

**Checkpoint**: Every PR builds and tests holzi on Android; `pending` lists what later stages fix.

---

## Phase 4: User Story 1 - holzi auf dem Telefon installieren, Tresor öffnen, Passwörter nutzen (P1) 🎯 MVP — Stage 1c

**Goal**: Phone UI, file choice without paths, vault file import (desktop and Android), "not available" everywhere, screen capture protection.

**Independent Test**: Quickstart §3 on a real phone including spec 036 T081; the new Android scenarios are green.

### Tests for User Story 1

- [ ] T032 [P] [US1] Rust tests first: `src-tauri/src/files/picked_tests.rs` (read/copy_into/write with desktop paths; the app-storage check accepts paths inside, refuses outside and `..`), `src-tauri/src/instances/cleanup_tests.rs` (rollback removes `.db`, `.db.pending`, `.db.lock`, `-wal`, `-shm`, `.db.vault-id`, `.tmp`), `src-tauri/src/instances/import_tests.rs` (all cases of contract import-instance.md: success, wrong passphrase, not a vault, already on this device, name conflict `-2`, nothing left after failure, original unchanged by checksum)
- [ ] T033 [P] [US1] e2e scenarios first (they fail): `scripts/e2e/scenarios/vault-file-import.test.ts` (desktop and Android), and Android-only `android-phone-layout`, `android-back-gesture`, `android-screen-capture`, `android-not-available`, `android-close-ends-app`, `android-process-death` under `scripts/e2e/scenarios/` as described in contract e2e-android.md; Android-only scenarios are skipped on Linux by a platform tag read in `scripts/e2e/lib/scenario.ts`

### Implementation for User Story 1

- [ ] T034 [US1] `src-tauri/src/files/mod.rs` and `src-tauri/src/files/picked.rs` per contract picked-file.md (`read`, `open_read`, `copy_into`, `write`, `display_name`, app-storage path check on Android, errors `Unreadable`/`NotEnoughSpace` in `src-tauri/src/error.rs`)
- [ ] T035 [US1] `src/composables/usePickedFile.ts` (open/save dialog → `PickedFile` and display name; never derives a path) and `src/composables/useDeviceCapabilities.ts` (`platform_capabilities` once at start, kept in memory)
- [ ] T036 [P] [US1] Passwords attachments: the add-attachment command in `src-tauri/src/passwords/` takes `PickedFile` and reads through `files::picked`; `src/components/passwords/Attachments.vue` uses `usePickedFile`
- [ ] T037 [P] [US1] Chat attachments: `src-tauri/src/chat/attachments.rs` command takes `PickedFile`; `src/components/chat/ComposerAttachments.vue` uses `usePickedFile`
- [ ] T038 [P] [US1] haex-vault import (spec 037): import commands in `src-tauri/src/passwords/import/` take `PickedFile`; on Android the companion file next to the vault file is not read and `src/components/passwords/ImportWizard.vue` says so (contract picked-file.md); desktop behaviour unchanged
- [ ] T039 [P] [US1] Appearance import/export and background: commands take `PickedFile` (save via `write`), `src/components/settings/AppearanceFileButtons.vue` uses `usePickedFile`
- [ ] T040 [P] [US1] Extension install from file: command takes `PickedFile`; `src/components/extensions/InstallDialog.vue` uses `usePickedFile`
- [ ] T041 [P] [US1] Model import from file: `src-tauri/src/models/commands.rs` `import_model_from_file(file: PickedFile)` copies through `files::picked::copy_into`; adjust its caller in `src/components/models/`
- [ ] T042 [US1] `src-tauri/src/instances/cleanup.rs` (one rollback helper for all sidecar files) used by `create.rs` (replaces its own rollback, which leaves `.db.lock`) and later by `import.rs`; `src-tauri/src/instances/vault_id.rs` writes `<name>.db.vault-id` (SHA-256 hex of the vault identity public key) on create and open; `src-tauri/src/instances/list.rs` skips `.vault-id` files; deleting a vault removes it (data-model.md)
- [ ] T043 [US1] `src-tauri/src/instances/import.rs` and command `import_instance` per contract import-instance.md (name, marker, copy, unlock with a variant of `open_instance_core` that accepts its own marker, `AlreadyOnThisDevice` via `.vault-id`, publish, session services as in `open.rs`); new error kinds in `src-tauri/src/error.rs`; register in `lib.rs`; `pnpm generate:ts-types`; make T032 pass
- [ ] T044 [US1] Vault picker: `src/pages/index.vue` gets the weaker „Tresordatei öffnen“ button next to „Neuer Tresor“ (emphasised) and „Verknüpfen“; new `src/components/onboarding/ImportVaultSheet.vue` (file name, passphrase, „Öffnen“, errors stay in the sheet); `src/composables/useInstance.ts` `importAsync`; i18n `onboarding.import.*` in `src/i18n/locales/de.json` and `en.json`
- [ ] T045 [US1] Plugin crate insets (contract android-plugin.md): in `HolziAndroidPlugin.kt` `load(webView)` set an `OnApplyWindowInsetsListener` writing `--holzi-inset-top|right|bottom|left|keyboard` (CSS px) on `document.documentElement`
- [ ] T046 [US1] Frontend insets: the wm root (`src/components/wm/Desktop.vue` and the vault picker page) pads by `var(--holzi-inset-*, 0px)`; new `src/composables/useKeyboardInset.ts` scrolls the focused field into view when `--holzi-inset-keyboard` changes (FR-011, FR-012)
- [ ] T047 [US1] Back gesture: `src/plugins/actions.client.ts` registers the `back-button` listener when `useDeviceCapabilities().backGesture` (replaces the user-agent check) and keeps the spec 020 FR-019 behaviour; the app is never left (FR-013)
- [ ] T048 [US1] Screen capture protection, Rust: plugin method `set_secure(enabled)` (Kotlin: `window.addFlags/clearFlags(FLAG_SECURE)` on the UI thread, `setRecentsScreenshotEnabled(!enabled)` on API 33+; Rust `mobile.rs`/`desktop.rs`); `src-tauri/src/privacy/screen_capture.rs` (device pref `privacy.screenCaptureProtection`, default on, pattern of `extensions/dev.rs`) applied after create/open/import and on change; tests in `src-tauri/src/privacy/screen_capture_tests.rs` (missing value = on, parse, scope is the device)
- [ ] T049 [US1] Screen capture setting UI: `src/components/settings/ScreenCaptureSetting.vue` in `src/components/settings/GeneralView.vue` (only when `useDeviceCapabilities().screenCapture`, "gilt nur für dieses Gerät" as in `SttModelSetting.vue`, no save button), registry entry in `src/lib/settings/registry.ts`, action in `src/stores/settingsActionHandlers.ts`, i18n; extend `scripts/check-settings.ts`
- [ ] T050 [US1] e2e Android platform turns screen capture protection off for every device after its first unlock (`scripts/e2e/lib/platform/android.ts`), except in `android-screen-capture`
- [ ] T051 [US1] "Not available" in the UI and the core (contract platform-capabilities.md, FR-016, FR-026): `src-tauri/src/adapters/cli_delegate/process.rs` checks `cliDelegates` first (`AdapterError::Unavailable` with reason `platform`); `src-tauri/src/chat/session.rs` registers `run_command` only with `commandTool`; `src/composables/useModelInventory.ts` hides delegates; settings `agents.providers`, `agents.autonomy`, `agents.denyRules` show synced delegate settings marked "Auf diesem Gerät nicht verfügbar"; extension dev mode switch hidden; folder choice in extension dialogs hidden; i18n `errors.notAvailable`; tests next to each Rust change
- [ ] T052 [US1] Stage 1c checkpoint: remove the `pending` entries Stage 1c fixed; run the Android scenarios of T033 and the whole Android suite locally; quickstart §3 on a real phone — tick spec 036 T081 in `specs/036-password-redesign/tasks.md` with the result; PR „feat(android): phone UI, chosen files and vault file import“

**Checkpoint**: US1 works on a real phone; spec 036 T081 is done.

---

## Phase 5: User Story 2 (continued) - Signierte Release-APKs (P1) — Stage 1d

**Goal**: Release APKs are signed with holzi's own key and install as updates.

**Independent Test**: Quickstart §9.

- [ ] T053 [US2] `src-tauri/gen/android/app/build.gradle.kts`: `signingConfigs.create("release")` from `rootProject.file("keystore.properties")` only when the file exists (Tauri docs pattern); release build type uses it; confirm `gen/android/.gitignore` excludes `keystore.properties`
- [ ] T054 [US2] `.github/workflows/android-release.yml` (tag `v*` and `workflow_dispatch`) per contract ci.md: write keystore and `keystore.properties` from `ANDROID_KEY_BASE64`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD` into `$RUNNER_TEMP`, fail when a secret is missing, build release APKs (`--target aarch64 --target x86_64`), `apksigner verify --print-certs` against `vars.ANDROID_CERT_SHA256`, attach APKs to the release (tag runs only); `pnpm check:android-versions` covers this file
- [ ] T055 [P] [US2] `README.md` section „Android“: devShell, build, install via adb or CI artifact, dev mode, release key handling by the operator (create once, keep safe, losing it ends updates; the three secrets and the public fingerprint variable)
- [ ] T056 [US2] Stage 1d checkpoint: with the operator's key, quickstart §9 (signature, update over the previous release keeps vaults, SC-008); PR „ci(android): signed release APKs“

---

## Phase 6: User Story 3 - Telefon mit dem Desktop synchronisieren (P2) — Stage 2

**Goal**: Pairing and sync between phone and desktop, foreground only, resilient to network changes.

**Independent Test**: Quickstart §5; `sync-*` and the other multi-device scenarios green with `phone` on the emulator.

- [ ] T057 [P] [US3] Tests first: `src-tauri/src/sync/resume_tests.rs` (the resume trigger calls the existing reconnect path once per resume and not while closing), `src-tauri/src/hardware/hostname_tests.rs` (device name from the plugin wins over sysinfo; `localhost` is never used as alias)
- [ ] T058 [US3] `src-tauri/src/lib.rs`: on `RunEvent::Resumed` call the existing reconnect/catch-up entry point of the sync (found with T002's graphify note) through a new `src-tauri/src/sync/resume.rs`; nothing runs on `Suspended` (FR-018: no background sync)
- [ ] T059 [US3] Plugin `network-changed` event: Kotlin `ConnectivityManager.registerDefaultNetworkCallback` in `HolziAndroidPlugin.kt` (permission `ACCESS_NETWORK_STATE` in the plugin manifest); Rust listener calls iroh `Endpoint::network_change()` and rebuilds the nostr WebSocket connections (FR-019, research R5)
- [ ] T060 [US3] `src-tauri/src/hardware/hostname.rs`: on Android use plugin `device_name()` (`Settings.Global.DEVICE_NAME`, else `Build.MODEL`) for `admission::adopt_computer_name` (research R10)
- [ ] T061 [US3] Remove the Stage 2 `pending` entries (`sync-*`, `*-two-devices`, `appearance-sync-two-devices`, `storage-two-devices`); fix what the mixed groups reveal; record in research.md R5 whether iroh detects network changes by itself on Android
- [ ] T062 [US3] Stage 2 checkpoint: quickstart §5 with a real phone and a desktop (SC-005 timing recorded); PR „feat(android): sync with the desktop“

---

## Phase 7: User Story 4 - Erweiterungen auf dem Telefon (P3) — Stage 3

**Goal**: haextensions install and run on Android with the same rules; frames never see holzi's internals.

**Independent Test**: Quickstart §6; `extension-*` scenarios green on the emulator (except the two `excluded`).

- [ ] T063 [P] [US4] Tests first: `src-tauri/src/extensions/protocol/csp_tests.rs` — the app origin in the CSP is `http://tauri.localhost` on Android and Windows and `tauri://localhost` elsewhere, from one source; dialog bridge tests in `src-tauri/src/extensions/fs/dialogs_tests.rs` — open returns the content of a chosen file, save writes through the chosen file, no path is ever returned to the extension on Android
- [ ] T064 [US4] `src-tauri/src/extensions/protocol/csp.rs` and `protocol/mod.rs`: derive the app origin per platform from one function (research R16)
- [ ] T065 [US4] `src-tauri/src/extensions/fs/dialogs.rs`: open/save through `files::picked` (`FilePath` instead of `into_path()`), on all platforms (FR-022)
- [ ] T066 [US4] Notifications: request `POST_NOTIFICATIONS` through the forked `tauri-plugin-notification` on the first notification an extension sends; when denied, keep the in-app notice and point to the Android setting (FR-023, research R12)
- [ ] T067 [US4] Remove the Stage 3 `pending` entries for `extension-*`; extend `android-not-available` with the counter cases of `extension-files` and `extension-dev-mode`; tick the Android part of spec 017 T112 in `specs/017-extension-host/tasks.md` (iOS stays open) with the emulator result of `extension-isolation`
- [ ] T068 [US4] Stage 3 checkpoint: quickstart §6 on a real phone; PR „feat(android): extensions on Android“

---

## Phase 8: User Story 5 - Chat mit Online-Anbietern auf dem Telefon (P4) — Stage 4

**Goal**: Chat with API providers on Android with proper certificate checks; delegates not offered.

**Independent Test**: Quickstart §7; chat scenarios green on the emulator.

- [ ] T069 [P] [US5] e2e first: `scripts/e2e/scenarios/chat-provider-untrusted-cert.test.ts` (desktop and Android) — the stand-in provider in `scripts/e2e/lib/provider.ts` gets an HTTPS mode with a self-signed certificate; sending a message shows a certificate error and no answer (FR-024)
- [ ] T070 [US5] Map TLS certificate failures of the provider adapters (`src-tauri/src/adapters/`) to a distinct error kind shown in the chat as „Zertifikat ungültig“ (i18n) instead of a generic network error
- [ ] T071 [US5] Remove the Stage 4 `pending` entries (chat scenarios); confirm on the emulator that `chat-model-search` shows no delegates (T051)
- [ ] T072 [US5] Stage 4 checkpoint: quickstart §7 on a real phone with a real provider; PR „feat(android): chat with online providers“

---

## Phase 9: User Story 6 - Lokale KI und Spracheingabe auf dem Telefon (P5) — Stage 5

**Goal**: Phone presets for local models, confirmation with size and free space before every model download (all platforms), speech input with microphone permission.

**Independent Test**: Quickstart §8 on a phone with ≥ 6 GB RAM.

- [ ] T073 [P] [US6] Tests first: `src-tauri/src/catalog/profiles_tests.rs` (mobile picks only `mobile`/`mobile_low_memory` models of `_meta.profiles`, choice by memory fit, desktop unchanged), `src-tauri/src/stt/catalog_tests.rs` (mobile default `whisper-tiny`), `src-tauri/src/models/download_check_tests.rs` (`fits` with 10 % reserve, missing size handled)
- [ ] T074 [US6] `src-tauri/src/catalog/profiles.rs`: read `_meta.profiles` from `catalog/model_catalog.json`; `catalog/mod.rs` recommendation uses it on mobile (FR-027)
- [ ] T075 [P] [US6] `src-tauri/src/stt/catalog.rs`: mobile default `whisper-tiny`, larger ones selectable (FR-030)
- [ ] T076 [US6] `src-tauri/src/models/download_check.rs` and command `model_download_check(model) -> { sizeBytes, freeBytes, fits }` (free space of the data directory via `sysinfo::Disks`) (FR-028)
- [ ] T077 [US6] `src/components/models/DownloadConfirmStep.vue` (size, free space, warning and „Trotzdem laden“ when it does not fit) used before every download in `DownloadModels.vue`, `ModelChoiceStep.vue`, `SttModelSetting.vue`, `SttModelChoiceStep.vue` (replaces its own size line) and the HuggingFace file picker; i18n; on desktop too (one rule, one place)
- [ ] T078 [US6] Microphone permission: plugin `request_permission(Microphone)` before the first capture in `src-tauri/src/voice.rs`/`src-tauri/src/audio/`; denied → error kind `MicrophoneDenied` with an explanation in the composer how to allow it later (FR-029)
- [ ] T079 [US6] e2e: `scripts/e2e/scenarios/models-download-confirm.test.ts` (desktop and Android) with a stand-in catalog entry served locally: the confirmation shows the size, nothing downloads before confirming, a too-large model shows the warning
- [ ] T080 [US6] Set the "pending must be empty" switch in `scripts/e2e/platform-exclusions.ts`; `pnpm check:e2e-exclusions` must pass with coverage ≥ 80 % (expected 95 %)
- [ ] T081 [US6] Stage 5 checkpoint: quickstart §8 on a phone with ≥ 6 GB RAM; measure SC-009 and write the result into `docs/adr/0010-android-platform.md` and research.md R11; if it misses 15 s, make Qwen3-0.6B the phone suggestion and record that; PR „feat(android): local models and speech input“

---

## Phase 10: Polish & Cross-Cutting

- [ ] T082 [P] `CONTEXT.md` glossary: „gewählte Datei“, „Fähigkeiten des Geräts“, „Bildschirmschutz“, „Tresordatei öffnen“; `plans/README.md` row for spec 043 with its status
- [ ] T083 [P] When lettre releases the fix of T009: drop the `[patch.crates-io]` entry and bump `lettre`; same for the wry patch once spec 017 T112's condition (upstream release) is met
- [ ] T084 Run the full [quickstart.md](./quickstart.md) and record the results per section in this task's note; tick SC-001 to SC-009 with measured values in `checklists/requirements.md` notes
- [ ] T085 PR texts for each stage list the stories, FRs and quickstart sections covered (no agent attribution)

---

## Dependencies & Execution Order

- **Phase 1** → **Phase 2** (Stage 1a) blocks everything.
- **Stage 1b** (Phase 3) needs Stage 1a (APK, `platform_capabilities`). From here the Android run is a required check.
- **Stage 1c** (Phase 4) needs Stage 1b for its e2e scenarios; T034 before T036–T041 and T043; T042 before T043; T045 before T046; T048 before T049–T050; T035 before every UI task.
- **Stage 1d** (Phase 5) needs only Stage 1a; it can run in parallel to 1b/1c in a second worktree, ships after 1c.
- **Stage 2** (Phase 6) needs Stage 1b (mixed groups) and the plugin crate.
- **Stage 3** (Phase 7) needs Stage 1c (`files::picked`, capability table).
- **Stage 4** (Phase 8) needs Stage 1a (TLS) and 1c (T051).
- **Stage 5** (Phase 9) needs Stage 1c (plugin permissions pattern); T080 runs last of all stages.
- **Polish** after the stages that ship.
- Tasks that edit `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`, `src/i18n/locales/*.json`, `.github/workflows/ci.yml`, `scripts/e2e/lib/platform/android.ts` or `HolziAndroidPlugin.kt` run one after the other across stories. Within a story: tests first (they fail), then implementation.

## Parallel Opportunities

- Phase 2: T005, T010, T014, T015 in parallel after T004; T007 after T006; T009 independent of all others (separate repository).
- US2 (1b): T022, T023, T025 in parallel after T020–T021; T030 any time.
- US1: T032 and T033 together; T036–T041 in parallel after T034/T035 (different files, each its own command).
- US2 (1d): T055 in parallel to T053–T054.
- US3: T057 first; T059 and T060 in parallel.
- US6: T075 in parallel to T074/T076.

## Implementation Strategy

- **MVP** = Stages 1a–1d (Phases 2–5, US1 + US2): holzi on a real phone with vault, passwords, phone UI, vault file import, and a CI that builds, tests on Android and signs releases.
- Then one PR per stage in priority order (Sync → Erweiterungen → Online-Chat → Lokale KI). Each stage removes its `pending` entries and ends with its quickstart section on a real phone.
- Operator steps outside the code: required check `android` (after Stage 1b), release key and secrets (Stage 1d).
