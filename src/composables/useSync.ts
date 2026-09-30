import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { SyncServers } from '@bindings/SyncServers'
import type { SyncStatus } from '@bindings/SyncStatus'
import type { VaultPublicIdentity } from '@bindings/VaultPublicIdentity'

const SYNC_DATA_CHANGED = 'sync-data-changed'

interface SyncDataChangedPayload {
  tables: string[]
}

/**
 * Live sync state for the open vault session (spec 024, FR-032): received
 * changes must appear in open windows and tabs without reloading. One
 * listener for the whole session — `startListening` is idempotent, called
 * once from `pages/workspace/[instance].vue`'s `onMounted` (the same spot
 * `useModelDownloadsStore().watchDownloads()` does it), and, like that
 * subscription, never torn down: it lives until the process ends, which is
 * the end of the vault session (one vault per process, spec 013).
 *
 * Views interested in a specific table watch `lastChangedTables`/
 * `changeCount` themselves (e.g. `watch(changeCount, () => { if
 * (lastChangedTables.value.includes('chat_threads')) refreshThreads() })`)
 * rather than `useSync` calling into every consumer directly. `changeCount`
 * goes up by one per event, so two events in quick succession still wake a
 * watcher twice.
 */
const lastChangedTables = ref<string[]>([])
const changeCount = ref(0)
let listening: Promise<void> | null = null

/** The session-wide sync change state, plus the call that starts it. */
export function useSync() {
  /** Registers the one `sync-data-changed` listener; later calls reuse it. */
  function startListening(): Promise<void> {
    // Shared while `listen` is pending, so a second caller does not
    // register a second listener; a failed registration may be retried.
    listening ??= listen<SyncDataChangedPayload>(SYNC_DATA_CHANGED, (event) => {
      lastChangedTables.value = event.payload.tables
      changeCount.value += 1
    }).then(
      () => {},
      (error: unknown) => {
        listening = null
        throw error
      },
    )
    return listening
  }

  /** Who this device is in the vault, open requests, and a link in progress. */
  async function syncStatusAsync(): Promise<SyncStatus> {
    return await invoke<SyncStatus>('sync_status')
  }

  /** The public key of the vault identity, as `npub` and hex. */
  async function vaultPublicIdentityAsync(): Promise<VaultPublicIdentity> {
    return await invoke<VaultPublicIdentity>('vault_public_identity')
  }

  /** The servers devices find each other through; an empty list means the defaults. */
  async function syncServersGetAsync(): Promise<SyncServers> {
    return await invoke<SyncServers>('sync_servers_get')
  }

  async function syncServersSetAsync(args: SyncServers): Promise<void> {
    await invoke('sync_servers_set', { args })
  }

  /** Removes a device from the vault (main device only, never this one). */
  async function deviceRemoveAsync(devicePubkey: string): Promise<void> {
    await invoke('device_remove', { args: { devicePubkey } })
  }

  /** Admits (`admit`) or refuses the request of a copy of the vault file (main device only). */
  async function admissionDecideAsync(
    devicePubkey: string,
    admit: boolean,
  ): Promise<void> {
    await invoke('admission_decide', { args: { devicePubkey, admit } })
  }

  /** The user has read that this copy enrolled itself as a main device. */
  async function copyNoticeDismissAsync(): Promise<void> {
    await invoke('sync_copy_notice_dismiss')
  }

  return {
    lastChangedTables,
    changeCount,
    startListening,
    syncStatusAsync,
    vaultPublicIdentityAsync,
    syncServersGetAsync,
    syncServersSetAsync,
    deviceRemoveAsync,
    admissionDecideAsync,
    copyNoticeDismissAsync,
  }
}
