import { computed, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import type { ExtensionSummary } from '@bindings/ExtensionSummary'
import { extensionApps } from '~/lib/extensions/apps'

/**
 * The installed extensions of the vault (spec 017, T043): the list for the launcher, the settings
 * and the app list of the window manager. It reloads on `extensions-changed` and
 * `extension-status-changed`; icons are fetched once per extension.
 */
export const useExtensionsStore = defineStore('extensions', () => {
  const list = ref<ExtensionSummary[]>([])
  const icons = reactive<Record<string, string>>({})
  /** `true` once the list was read from the vault (a session restore waits for it). */
  const loaded = ref(false)
  let unlisten: UnlistenFn[] = []

  const apps = computed(() => extensionApps(list.value, icons))

  async function loadIconAsync(extensionId: string): Promise<void> {
    if (icons[extensionId]) return
    try {
      const url = await invoke<string | null>('extension_icon', { extensionId })
      if (url) icons[extensionId] = url
    } catch {
      // No icon: the launcher shows the default one.
    }
  }

  async function loadAsync(): Promise<void> {
    list.value = await invoke<ExtensionSummary[]>('extension_list')
    loaded.value = true
    await Promise.all(
      list.value.filter((e) => e.hasIcon).map((e) => loadIconAsync(e.id)),
    )
  }

  /** Loads the list and follows changes until `stop`. */
  async function startAsync(): Promise<void> {
    if (unlisten.length > 0) return
    unlisten = await Promise.all([
      listen('extensions-changed', () => void loadAsync()),
      listen('extension-status-changed', () => void loadAsync()),
    ])
    await loadAsync()
  }

  function stop(): void {
    for (const off of unlisten) off()
    unlisten = []
  }

  return { list, icons, loaded, apps, loadAsync, startAsync, stop }
})
