# Quickstart: Portable Mode with Protected Data

This guide is executable only after the feasibility gate has selected a supported
protected-container implementation. Until then, run the path-isolation checks
and document the single-file mode as unavailable.

## Automated checks

```bash
cargo test --manifest-path src-tauri/Cargo.toml portable
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm check:portable-paths
pnpm check:portable-filesystem
pnpm check:templates
pnpm typecheck && pnpm typecheck:scripts
pnpm lint && pnpm format:check
```

Expected: normal-installation tests remain green; portable path checks show every
app-owned path below the selected root; unavailable roots create no host fallback.

## Windows removable-storage scenario

1. Copy the portable executable and an empty data root to removable storage.
2. Start it on a clean Windows account without administrator rights.
3. Record the computer's app-data, temp, cache, recent-file, and startup state.
4. Create/unlock a vault, chat with a local model, import a marker file, use a
   tool, close the vault, and relaunch.
5. Verify the mode and root survive relaunch, then remove the storage.
6. Scan the computer for app-created files; only OS-managed traces named in the
   limits statement may remain.

## Protected-data scenario

1. Put unique markers in vault content, settings, model metadata, model bytes,
   imported files, and cache entries.
2. Close the session and inspect the raw root without the passphrase.
3. Verify no marker, model name, or vault name is readable where the contract
   promises protection.
4. Unlock with the correct passphrase and chat with the local model.
5. Repeat with a wrong passphrase and a corrupted copy; no protected data may be
   returned.

## Windows single-file scenario

1. Run the non-installer executable without elevation.
2. Create a container in a user-selected directory and verify free-space refusal.
3. Confirm the container name is opaque and all app-owned data is inside it.
4. Close and relaunch from the same executable; unlock the same container.
5. Accept the delete prompt and verify the app-owned container is gone while the
   documented filesystem-recovery limitation remains true.

## Limits statement review

Before unlock, verify the UI names protection against the next user and storage
loss, but not keyloggers, screen capture, memory inspection, paging,
hibernation, crash reports, recent-file records, download/security scans, device
logs, visible container metadata, or deliberate user-requested host writes.
