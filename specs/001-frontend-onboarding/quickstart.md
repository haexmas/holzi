# Quickstart: Frontend Onboarding

**Goal (post-implementation)**: after the frontend scaffold, `haex-crdt` extraction, and the implementation tasks are complete, reach a running Tauri window from a fresh clone of `holzi` that renders the landing page with the three primary CTAs (Anlegen, Öffnen, Verbinden) and the "Zuletzt verwendet" list, in under 5 minutes on a warm machine.

**Prerequisites** on the host:

- Node.js LTS + `pnpm` in `PATH`.
- Rust toolchain matching `rust-toolchain.toml` (once added — the extraction task provides this).
- Tauri build prerequisites for the host OS (Linux: `webkit2gtk-4.1`; macOS: Xcode CLI Tools; Windows: MSVC + WebView2).
- `haex-crdt` crate available (see [`v1-scope-design.md §10`](../../docs/plans/2026-09-04-v1-scope-design.md) — this is currently a blocker; use a workspace-path override if the crate is not yet published).

## Steps

1. **Clone and install dependencies**.

   ```bash
   git clone git@github.com:haexmas/holzi.git
   cd holzi
   pnpm install
   ```

2. **Verify Nuxt + Tailwind + shadcn-vue setup**.

   ```bash
   pnpm dev
   ```

   Open the dev URL (default `http://localhost:3000`) — you should see the landing shell, three CTA buttons (disabled or empty labels until i18n keys are filled), and the version footer.

3. **Run the Tauri app**.

   ```bash
   pnpm tauri dev
   ```

   A native window opens with the same page. Verify:

   - No network requests to `api.iconify.design` or `fonts.googleapis.com` (open DevTools → Network, refresh).
   - No console errors.
   - "Zuletzt verwendet" section is hidden (empty `instances/` directory).

4. **Create a first instance (Anlegen — Genesis)**.

   - Click **Anlegen**.
   - Choose "Neue Federation".
   - Name: `test-01` (any alphanumeric name).
   - Passphrase: any string meeting the min-length policy, entered twice.
   - Submit.
   - Paper-seed is displayed. **Read it, physically record it, tick the confirmation.**
   - Click Continue.
   - App navigates to `/federation/test-01` (placeholder page for this spec).

   Verify on disk:

   ```bash
   ls -la ~/.local/share/holzi/instances/    # Linux path — adjust per OS
   # Expect: test-01.db
   ```

5. **Return to landing and unlock**.

   - Navigate back to `/` (a "Zur Übersicht"-button will be added in a later spec; for now use the browser back button in `pnpm dev`, or restart `pnpm tauri dev`).
   - The instance `test-01` appears in "Zuletzt verwendet".
   - Click it → Unlock sheet opens → enter the passphrase → arrive at `/federation/test-01`.

6. **Import an external `.db` (Öffnen)**.

   - Copy `~/.local/share/holzi/instances/test-01.db` to `/tmp/other.db`.
   - In the app, click **Öffnen**.
   - File picker opens; select `/tmp/other.db`.
   - Import succeeds; `other.db` appears in the list (source at `/tmp/other.db` is untouched).

7. **Verbinden requires two devices** — end-to-end pairing is out of scope for this quickstart. To smoke-test the flow with a single machine, run two `pnpm tauri dev` instances against separate `AppLocalData` roots (via `XDG_DATA_HOME` on Linux). Follow-up docs will cover this.

## Common failure modes

- **Blank window in Tauri**: Nuxt dev server not yet up. Wait for `pnpm dev` in another terminal, or use `pnpm tauri dev` which starts both.
- **`instances/` not created**: backend calls `create_dir_all` on first path resolution — check the app has write permission to `AppLocalData`.
- **"Öffnen fehlgeschlagen" after correct passphrase**: verify `haex-crdt` version matches the one used to create the DB. Migration is out of scope for this spec.
- **Icons render as boxes**: `@iconify-json/lucide` not installed, or `nuxt.config.ts` icon settings not committed. Re-run `pnpm install` and restart dev server.

## Definition of "onboarding-complete"

This spec is done when:

- A fresh clone reaches step 5 (create + unlock loop) without deviations.
- Step 6 (Öffnen) works with an arbitrary valid `.db`.
- All E2E tests in `e2e/onboarding.spec.ts` pass.
- Playwright network-assertion test (T066) passes with zero external requests.
