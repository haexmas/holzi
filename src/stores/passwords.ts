import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { GroupRow } from '@bindings/GroupRow'
import type { ItemHeader } from '@bindings/ItemHeader'
import type { TagRow } from '@bindings/TagRow'

/**
 * The overview of the password manager (spec 034, research R15): every entry header, folder and
 * tag the window shows, loaded by one command and reloaded quietly when a table changes — through
 * another window, an agent, an extension or a sync. The store holds headers only, never a secret
 * (FR-040); a secret is fetched for the moment the user asks and kept in a component-local ref.
 *
 * A quiet reload replaces the lists but never touches a draft: drafts are local to the editor.
 */
export const usePasswordsStore = defineStore('passwords', () => {
  const { loadOverviewAsync } = usePasswords()
  const { errString } = useErrorString()

  const headers = ref<ItemHeader[]>([])
  const groups = ref<GroupRow[]>([])
  const tags = ref<TagRow[]>([])
  const isLoading = ref(false)
  const hasLoadedOnce = ref(false)
  const lastError = ref<string | null>(null)

  /** Entries by id, for the places that hold only an id (every place carries ids only). */
  const headersById = computed(
    () => new Map(headers.value.map((header) => [header.id, header])),
  )

  async function loadAsync() {
    const overview = await loadOverviewAsync()
    headers.value = overview.headers
    groups.value = overview.groups
    tags.value = overview.tags
    lastError.value = null
    hasLoadedOnce.value = true
  }

  /** The first load and an explicit reload: shows the loading state. */
  async function reloadAsync() {
    isLoading.value = true
    try {
      await loadAsync()
    } catch (error) {
      lastError.value = errString(error)
    } finally {
      isLoading.value = false
    }
  }

  /** The reload a change event triggers: no spinner, no flash, an error is kept for the banner. */
  async function quietReloadAsync() {
    try {
      await loadAsync()
    } catch (error) {
      lastError.value = errString(error)
    }
  }

  onVaultTablesChanged(['haex_passwords_*'], quietReloadAsync)

  return {
    headers,
    groups,
    tags,
    headersById,
    isLoading,
    hasLoadedOnce,
    lastError,
    reloadAsync,
    quietReloadAsync,
  }
})
