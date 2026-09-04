# Contract: Tauri Events (Backend → Frontend)

**Status**: Draft. Event names and payload shapes are working proposals.

Backend emits events via `AppHandle::emit(topic, payload)`. Frontend subscribes via `@tauri-apps/api/event::listen(topic, handler)`.

## Events

### `instance-list-changed`

Emitted whenever the set of files in `<AppLocalData>/instances/` changes as a result of a backend command: create, open (last-access bump), close (active-state transition), import, trash. Also emitted on startup after any orphan-cleanup pass and for changes made outside the Tauri command handlers.

**Payload**:

```ts
type InstanceListChanged = {
  reason: 'created' | 'opened' | 'closed' | 'imported' | 'trashed' | 'aborted' | 'startup-cleanup' | 'external-change'
  affectedName?: string   // Present for single-instance mutations
  source?: 'external-directory-watcher' | 'android-share-intent'
}
```

**Consumer behavior**:

The `useInstancesStore` Pinia store subscribes in its factory and calls `syncAsync()` on every event. The `reason` and optional `source` fields are intended for future telemetry and for tests to assert cause; the store does not branch on them.

**Why an event and not polling**:

Mutations can originate outside the drawer UI (Android share-intent, CLI, or another process editing the directory). At startup the backend registers a debounced watcher for the instances directory. A create, remove, or rename observed from the CLI or another process triggers a rescan and one `external-change` event with `source: 'external-directory-watcher'`. The Android share-intent handler copies the selected file through the same backend-owned destination path and emits an `external-change` event with `source: 'android-share-intent'` after the copy succeeds. The watcher is the fallback for share-intent implementations that materialize a file directly. Polling would either be wasteful or laggy. haex-vault uses the same pattern for `vault-list-changed` (see [`stores/vault/lastVaults.ts:37`](../../../../haex-vault/src/stores/vault/lastVaults.ts#L37)).

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
