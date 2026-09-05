# Contract: Tauri Events (Backend → Frontend)

**Status**: Draft. Event names and payload shapes are working proposals.

Backend emits events via `AppHandle::emit(topic, payload)`. Frontend subscribes via `@tauri-apps/api/event::listen(topic, handler)`.

## Events

### `instance-list-changed`

Emitted whenever a backend command mutates the set of files in `<AppLocalData>/instances/`: create, confirm-create, abort-create, open (last-access bump), close (active-state transition), import, trash. Also emitted on startup after any orphan-cleanup pass.

**Payload**:

```ts
type InstanceListChanged = {
  reason: 'created' | 'opened' | 'closed' | 'imported' | 'trashed' | 'aborted' | 'startup-cleanup'
  affectedName?: string   // Present for single-instance mutations
}
```

**Consumer behavior**:

The `useInstancesStore` Pinia store subscribes in its factory and calls `syncAsync()` on every event. The `reason` field is intended for future telemetry and for tests to assert cause; the store does not branch on it.

**Why an event and not polling**:

Every mutation in v1 goes through a Tauri command, so the command handler is the authoritative producer. Polling would be wasteful. haex-vault uses the same pattern for `vault-list-changed` (see [`stores/vault/lastVaults.ts:37`](../../../../haex-vault/src/stores/vault/lastVaults.ts#L37)).

**Explicitly not in v1**: a filesystem watcher on the instances directory for changes made outside the app (CLI drop-in, third-party file managers). v1 has no CLI, no share-intent code path that bypasses `import_instance_file`, and no other in-scope mutator. If such a path is introduced later, the watcher (with a debounce and platform-specific backend) can be added then; the event shape above accommodates it without a breaking change.

### `active-instance-changed` *(v1 optional)*

Emitted when `AppState.active_instance` transitions between `Some` and `None`, or between two `Some` values (after a close-and-open cycle).

**Payload**:

```ts
type ActiveInstanceChanged = {
  previous?: InstanceInfo
  current?: InstanceInfo
}
```

**Consumer behavior**:

Currently not required by the landing surface, since `create_instance` and `open_instance` return the new active instance directly. Reserved for later stories where a background subsystem (e.g., a scheduled sync) might close the instance under the operator's feet.
