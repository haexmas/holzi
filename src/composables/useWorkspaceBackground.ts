import { readonly, ref } from 'vue'
import { usePreferences } from '~/composables/usePreferences'
import { downscaleToWebp } from '~/lib/images/downscale'
import {
  BACKGROUND_KEY,
  BACKGROUND_MAX_EDGE,
  isBackgroundValue,
} from '~/lib/settings/background'

/** One state per process: a process holds one vault (spec 013). */
const background = ref<string | null>(null)

function dataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
    reader.readAsDataURL(blob)
  })
}

/**
 * The workspace background (spec 042, FR-016–FR-018, research R6/R7): one image behind every
 * workspace, stored for the whole vault as a WebP data URL whose longer edge is at most
 * `BACKGROUND_MAX_EDGE` pixels. Only the user picks the file; an agent can only remove it.
 */
export function useWorkspaceBackground() {
  const { getPrefAsync, setPrefAsync, clearPrefAsync } = usePreferences()

  async function readAsync(): Promise<string | null> {
    const stored = await getPrefAsync({ kind: 'vault' }, BACKGROUND_KEY)
    return isBackgroundValue(stored) ? stored : null
  }

  /** Reads the vault's value once it is open; the previous vault's image is not shown meanwhile. */
  async function loadAsync(): Promise<void> {
    background.value = null
    background.value = await readAsync()
  }

  // ponytail: every change to `preferences` re-reads the whole value over IPC (a few hundred KB).
  // Ceiling: noticeable only with frequent preference writes. Upgrade path: the image in its own
  // table keyed by its hash, like `haex_passwords_binaries` (`passwords/binaries.rs`), and only
  // the hash in the preference.
  /** Re-reads after a synced change; keeps what is shown when nothing changed. */
  async function refreshAsync(): Promise<void> {
    const next = await readAsync()
    if (next !== background.value) background.value = next
  }

  /** Scales the chosen image down and stores it; an image that cannot be read changes nothing. */
  async function setFromFileAsync(file: File): Promise<void> {
    const scaled = await downscaleToWebp(
      await file.arrayBuffer(),
      file.type,
      BACKGROUND_MAX_EDGE,
    )
    const value = await dataUrl(scaled)
    if (!isBackgroundValue(value)) throw new Error('no WebP image')
    await setPrefAsync({ kind: 'vault' }, BACKGROUND_KEY, value)
    background.value = value
  }

  async function removeAsync(): Promise<void> {
    await clearPrefAsync({ kind: 'vault' }, BACKGROUND_KEY)
    background.value = null
  }

  return {
    background: readonly(background),
    loadAsync,
    refreshAsync,
    setFromFileAsync,
    removeAsync,
  }
}
