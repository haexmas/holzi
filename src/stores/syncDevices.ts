import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { SyncStatus } from '@bindings/SyncStatus'
import type { VaultDevice } from '@bindings/VaultDevice'
import type { VaultPublicIdentity } from '@bindings/VaultPublicIdentity'

const SYNC_DEVICES_CHANGED = 'sync-devices-changed'

/**
 * The vault's devices for the settings view (spec 024, user story 4, FR-033, FR-034): the list,
 * this device's place in the vault and the public identity. One listener for `sync-devices-changed`
 * reloads the list and the status, so a device coming online, a rename, a new or removed device
 * show without reloading. Events that arrive while a load runs are folded into one more load.
 */
export const useSyncDevicesStore = defineStore('syncDevices', () => {
  const { listVaultDevicesAsync } = useDevice()
  const { syncStatusAsync, vaultPublicIdentityAsync } = useSync()
  const { errString } = useErrorString()

  const devices = ref<VaultDevice[]>([])
  const status = ref<SyncStatus | null>(null)
  const identity = ref<VaultPublicIdentity | null>(null)
  const loading = ref(true)
  const error = ref<string | null>(null)

  let unlisten: UnlistenFn | null = null
  let pendingListen: Promise<void> | null = null
  let listenGeneration = 0
  let loadingNow = false
  let again = false

  /** Loads the list and the status; a call during a load asks for one more. */
  async function loadAsync(): Promise<void> {
    if (loadingNow) {
      again = true
      return
    }
    loadingNow = true
    try {
      do {
        again = false
        try {
          const [list, next] = await Promise.all([
            listVaultDevicesAsync(),
            syncStatusAsync(),
          ])
          devices.value = list
          status.value = next
          error.value = null
        } catch (e) {
          error.value = errString(e)
        }
      } while (again)
    } finally {
      loadingNow = false
      loading.value = false
    }
  }

  /** The identity never changes (FR-046), so it loads once. */
  async function loadIdentityAsync(): Promise<void> {
    if (identity.value) return
    try {
      identity.value = await vaultPublicIdentityAsync()
    } catch (e) {
      error.value = errString(e)
    }
  }

  async function startListening(): Promise<void> {
    if (unlisten) return
    if (pendingListen) return await pendingListen

    const generation = ++listenGeneration
    const registration = listen(SYNC_DEVICES_CHANGED, () => {
      void loadAsync()
    }).then((cleanup) => {
      if (
        generation !== listenGeneration ||
        pendingListen !== registration ||
        unlisten
      ) {
        cleanup()
        return
      }
      unlisten = cleanup
    })
    pendingListen = registration
    try {
      await registration
    } finally {
      if (pendingListen === registration) pendingListen = null
    }
  }

  function stopListening(): void {
    listenGeneration += 1
    unlisten?.()
    unlisten = null
    pendingListen = null
  }

  return {
    devices,
    status,
    identity,
    loading,
    error,
    loadAsync,
    loadIdentityAsync,
    startListening,
    stopListening,
  }
})
