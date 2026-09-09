import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { InstanceInfo } from '@bindings/InstanceInfo'

const INSTANCE_LIST_CHANGED = 'instance-list-changed'

interface InstanceListChangedPayload {
  reason: string
  affectedName: string | null
}

export const useInstancesStore = defineStore('instances', () => {
  const { listAsync } = useInstance()

  const instances = ref<InstanceInfo[]>([])
  const activeInstance = ref<string | null>(null)
  const lastError = ref<string | null>(null)

  let unlisten: UnlistenFn | null = null

  async function syncAsync() {
    try {
      instances.value = await listAsync()
      lastError.value = null
    }
    catch (e) {
      lastError.value = e instanceof Error ? e.message : String(e)
    }
    return instances.value
  }

  async function startListening() {
    if (unlisten)
      return
    unlisten = await listen<InstanceListChangedPayload>(
      INSTANCE_LIST_CHANGED,
      () => {
        void syncAsync()
      },
    )
  }

  function stopListening() {
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  }

  function setActiveInstance(name: string | null) {
    activeInstance.value = name
  }

  return {
    instances,
    activeInstance,
    lastError,
    syncAsync,
    startListening,
    stopListening,
    setActiveInstance,
  }
})
