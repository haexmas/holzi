import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { sniffImageType } from '~/lib/passwords/icons'

/**
 * The pictures the import stored for entries and folders (spec 034, T088): `binary:<hash>` icons
 * are loaded lazily through `passwords_icon_preview`, kept as blob URLs and revoked when the cache
 * is cleared (the window closes). A picture that cannot be loaded stays `null`, so the component
 * shows the default and the store never asks again for it.
 */
export const usePasswordsIconsStore = defineStore('passwordsIcons', () => {
  const urls = reactive(new Map<string, string | null>())
  const pending = new Set<string>()

  /** The blob URL of a loaded picture; `undefined` while it is not loaded, `null` if it failed. */
  function urlFor(hash: string): string | null | undefined {
    return urls.get(hash)
  }

  async function loadAsync(hash: string): Promise<void> {
    if (urls.has(hash) || pending.has(hash)) return
    pending.add(hash)
    try {
      const buffer = await invoke<ArrayBuffer>('passwords_icon_preview', {
        args: { hash },
      })
      const bytes = new Uint8Array(buffer)
      urls.set(
        hash,
        URL.createObjectURL(new Blob([bytes], { type: sniffImageType(bytes) })),
      )
    } catch {
      urls.set(hash, null)
    } finally {
      pending.delete(hash)
    }
  }

  function clear() {
    for (const url of urls.values()) if (url) URL.revokeObjectURL(url)
    urls.clear()
  }

  return { urlFor, loadAsync, clear }
})
