# Contract: Shared Types (Rust ↔ TypeScript)

**Status**: Draft. All types are generated from Rust `#[derive(ts_rs::TS)]` structs and exported to `src/types/bindings/` via `cargo test` (per haex-vault's `generate:ts-types` script).

Frontend imports via a `@bindings/*` path alias:

```ts
import type { InstanceInfo } from '@bindings/InstanceInfo'
```

## Types

### `InstanceInfo`

Metadata for one instance in the list. No secrets.

```rust
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub name: String,              // Filename basename without `.db`
    pub last_access: String,       // ISO-8601, from file mtime or a persisted last-open
    pub size_bytes: u64,           // File size at scan time
}
```

**TypeScript view**:

```ts
export type InstanceInfo = {
  name: string
  lastAccess: string
  sizeBytes: number
}
```

### `CreateMode`

```rust
#[derive(Debug, Clone, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum CreateMode {
    Genesis,
    Recover { seed: String, expected_fingerprint: String },
    Join { token: String },
}
```

**TypeScript view**:

```ts
export type CreateMode =
  | { type: 'Genesis' }
  | { type: 'Recover'; seed: string; expectedFingerprint: string }
  | { type: 'Join'; token: string }
```

### `ConflictPolicy`

```rust
#[derive(Debug, Clone, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub enum ConflictPolicy {
    Rename,
    Overwrite,
    Abort,
}
```

### `CreateInstanceArgs`, `CreateInstanceResult`, `OpenInstanceArgs`, `ImportInstanceArgs`, `ImportInstanceResult`, `TrashInstanceArgs`

See [`tauri-commands.md`](./tauri-commands.md). All derive `TS` with `#[serde(rename_all = "camelCase")]` for consistent camelCase field names on the wire.

### `HolziError`

See [`tauri-commands.md`](./tauri-commands.md) → "Error type". Serialized as a tagged union with `kind` as the discriminator, matching Rust's `#[serde(tag = "kind")]`.

## Generation flow

1. Rust structs live in `src-tauri/src/instances/types.rs`, `src-tauri/src/error.rs`, `src-tauri/src/pairing/types.rs`.
2. `#[derive(TS)]` + `#[ts(export)]` writes them to `src-tauri/bindings/` when `cargo test` runs (per `ts-rs` default; the `generate:ts-types` script wraps this).
3. A post-generation step (npm script or Rust build helper) copies or symlinks `src-tauri/bindings/` to `src/types/bindings/` so the frontend's `@bindings/*` alias resolves without walking out of `src/`.
4. CI has a check that the generated files under `src/types/bindings/` match a fresh regeneration — drift fails the build. haex-vault uses `test:constants` for the same purpose.

## Naming rules

- Rust struct names are `PascalCase`; TS exports the same names.
- Enum variants: `PascalCase` on both sides; `#[serde(tag = "type")]` for external-tagged discriminant `type` (data enums) and `#[serde(tag = "kind")]` for `HolziError` — chosen to keep error-vs-mode discriminators visually distinct in logs.
- Field names: Rust `snake_case`, wire `camelCase` (via `#[serde(rename_all = "camelCase")]`), TS `camelCase`.
