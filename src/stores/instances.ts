import { defineStore } from 'pinia'

export interface InstanceInfo {
  name: string
  alias: string | null
  lastAccess: number
}

/** Stores the known instances and the instance active in this process. */
export const useInstancesStore = defineStore('instances', () => {
  const instances = ref<InstanceInfo[]>([])
  const activeInstance = ref<string | null>(null)

  /** Returns known instances until the backend synchronization slice is connected. */
  async function syncAsync() {
    // Wired up in a later slice — calls invoke('list_instances').
    return instances.value
  }

  return {
    instances,
    activeInstance,
    syncAsync,
  }
})
