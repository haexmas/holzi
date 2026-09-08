# Quickstart: Frontend Onboarding

**Goal (post-implementation)**: after the frontend scaffold, `haex-crdt` extraction, and the implementation tasks are complete, reach a running Tauri window from a fresh clone of `holzi` that renders the landing page with the three primary CTAs (Anlegen, Öffnen, Verbinden) and the "Zuletzt verwendet" list, in under 5 minutes on a warm machine.

**V1 contract note**: Genesis creates a fresh per-instance identity without a
paper-seed display or confirmation step. Imported databases complete attested
adoption offline before network startup; token pairing is the fallback when no
valid attestation is available, including later invalidation.

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
   - Fresh Nostr and iroh identities are generated inside the encrypted database.
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

   - Close `test-01` cleanly before taking the copy, so committed data is not left in an active WAL. Copy `~/.local/share/holzi/instances/test-01.db` to `/tmp/other.db`; the local index may still know the source UUID, but that UUID cannot identify a second replica.
   - In the app, click **Öffnen**.
   - File picker opens; select `/tmp/other.db`.
   - Import succeeds; `other.db` appears in the list with import-pending state (source at `/tmp/other.db` is untouched).

7. **Verify the adoption gate; restore after the dependency upgrade**.

   - At the current crate pin, select `other.db` and enter the original passphrase. Verify the explicit adoption-unavailable `CrdtInit` error, retained copy and import marker, unchanged index and active instance, and no new endpoint. Repeat with a copy whose source UUID is unknown; the result must be the same.
   - The following success checks are blocked until an adoption-capable revision is reviewed and pinned. Only the token-fallback check requires a reachable parent with pairing authority. After that upgrade, verify that `open_instance` adopts a fresh UUID and signing/endpoint keys before any application write or endpoint starts, preserving the original source identity.
   - Verify that a handover attestation was written with the copied signing key **before** that key was retired, and that the app then opens `/federation/other` directly — with no network reachable, no token, and no parent running. Adoption completes offline (FR-011b).
   - Bring a peer whose effective trust store still authorizes the source grant online. Verify that it checks the complete attested identity, endpoint-key proof, capabilities, expiry and epochs before allowing ordinary traffic, and visibly shows the newly derived replica. A changed endpoint key or capability must be rejected; so must an expired or revoked grant even with a valid signature.
   - Restart an attested copy after interrupted index publication and after a forced endpoint startup failure. Verify the same UUID, keys and attestation, `restorePairingRequired = false`, no leftover import/restore marker after recovery, and no token prompt.
   - Fallback path only: with a copy whose Nostr signing key is missing/unusable or whose source lacks pairing authority, verify the app enters `restore-pairing-required`, offers **Wiederherstellung verbinden**, and that a valid parent token makes the federation view call `pair_restored_instance`, update the same `other.db` in place, remove the restore marker, and create no second database. Repeat after a receiver rejects an initially attested copy against newer revocation state; preserve the already adopted identity throughout token reauthorization.

8. **Verbinden requires two instances**. Token-Join creates a fresh database and is independent of the adoption gate. To test it (or the future restore-pairing checks in step 7) on one machine, run two `pnpm tauri dev` instances against separate `AppLocalData` roots (via `XDG_DATA_HOME` on Linux), using a reachable parent with pairing authority. Follow-up docs will cover the complete two-device setup.

## Common failure modes

- **Blank window in Tauri**: Nuxt dev server not yet up. Wait for `pnpm dev` in another terminal, or use `pnpm tauri dev` which starts both.
- **`instances/` not created**: backend calls `create_dir_all` on first path resolution — check the app has write permission to `AppLocalData`.
- **"Öffnen fehlgeschlagen" after correct passphrase**: verify `haex-crdt` version matches the one used to create the DB. Migration is out of scope for this spec.
- **Icons render as boxes**: `@iconify-json/lucide` not installed, or `nuxt.config.ts` icon settings not committed. Re-run `pnpm install` and restart dev server.

## Definition of "onboarding-complete"

This spec is done when:

- A fresh clone reaches step 5 (create + unlock loop) without deviations.
- Steps 6–7 stage both known-UUID and unknown-UUID copies safely and enforce the adoption gate at the current pin. Full import onboarding remains dependency-blocked until a reviewed crate upgrade makes fresh UUID/key adoption and offline attested completion pass for both cases, alongside the separate token-fallback checks; rejection tests alone do not satisfy that success path. See FR-011a.
- All E2E tests in `e2e/onboarding.spec.ts` pass.
- Playwright network-assertion test (T066) passes with zero external requests.
