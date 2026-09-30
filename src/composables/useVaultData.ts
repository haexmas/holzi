import { getCurrentScope, onScopeDispose } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { createVaultDataHub, type TablePattern } from '~/lib/sync/vaultData'

const VAULT_DATA_CHANGED = 'vault-data-changed'

interface VaultDataChangedPayload {
  tables: string[]
}

/**
 * Live vault data (spec 024 FR-032, for every writer): whenever tables of the open vault change
 * — a write from another window, an agent or an extension, or changes received from another
 * device — the backend sends one `vault-data-changed` event (`src-tauri/src/vault_events.rs`),
 * and every view that shows data from those tables reloads it, so nothing needs a restart or a
 * manual refresh to catch up.
 *
 * A view registers what it shows with {@link onVaultTablesChanged}. One listener serves the
 * whole session: `startListening` is idempotent and called once from
 * `pages/workspace/[instance].vue`; like the model-download subscription it is never torn down,
 * because it lives as long as the process, which is the vault session (spec 013).
 */
const hub = createVaultDataHub((error) => {
  console.error('[vault-data] reloading after a change failed', error)
})
let listening: Promise<void> | null = null

/** Registers the one `vault-data-changed` listener; later calls reuse it. */
export function startVaultDataListening(): Promise<void> {
  // Shared while `listen` is pending, so a second caller does not register a second listener;
  // a failed registration may be retried.
  listening ??= listen<VaultDataChangedPayload>(VAULT_DATA_CHANGED, (event) => {
    void hub.dispatch(event.payload.tables)
  }).then(
    () => {},
    (error: unknown) => {
      listening = null
      throw error
    },
  )
  return listening
}

/**
 * Runs `handler` whenever one of `tables` changes in the vault (a name, or a prefix ending in
 * `*`). Inside a component or store the subscription ends with it; elsewhere the returned
 * function ends it. The handler is called after the change is committed, never twice at once,
 * and once more when a change arrives while it runs. It should reload quietly: no spinner, and
 * no overwriting of what the user is editing.
 */
export function onVaultTablesChanged(
  tables: readonly TablePattern[],
  handler: () => void | Promise<void>,
): () => void {
  const unsubscribe = hub.subscribe({ tables, handler })
  if (getCurrentScope()) onScopeDispose(unsubscribe)
  return unsubscribe
}
