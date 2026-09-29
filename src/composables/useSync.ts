import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

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
 * `lastChangedAt` themselves (e.g. `watch(lastChangedAt, () => { if
 * (lastChangedTables.value.includes('chat_threads')) refreshThreads() })`)
 * rather than `useSync` calling into every consumer directly.
 */
const lastChangedTables = ref<string[]>([])
const lastChangedAt = ref(0)
let unlisten: UnlistenFn | null = null

export function useSync() {
  async function startListening() {
    if (unlisten) return
    unlisten = await listen<SyncDataChangedPayload>(
      SYNC_DATA_CHANGED,
      (event) => {
        lastChangedTables.value = event.payload.tables
        lastChangedAt.value = Date.now()
      },
    )
  }

  return {
    lastChangedTables,
    lastChangedAt,
    startListening,
  }
}
