import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'

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

export function useSync() {
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

  return {
    lastChangedTables,
    changeCount,
    startListening,
  }
}
