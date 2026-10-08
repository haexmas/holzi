import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import type { DialogFilter } from '@tauri-apps/plugin-dialog'
import type { PickedFile } from '~/types/bindings/PickedFile'

/**
 * The system's file dialogs (spec 043 FR-015, contract picked-file.md). A chosen file is a path on
 * a desktop and a `content://` address on Android; it goes to the backend unchanged, and the window
 * never derives a path or a name from it. On Android the filters are left out: document providers
 * do not report file name extensions reliably.
 */
export function usePickedFile() {
  const { readyAsync } = useDeviceCapabilities()

  async function filtersFor(filters?: DialogFilter[]) {
    const table = await readyAsync()
    return table?.platform === 'android' ? undefined : filters
  }

  /** One file, or `null` when the dialog was cancelled. */
  async function pickOneAsync(
    filters?: DialogFilter[],
  ): Promise<PickedFile | null> {
    const chosen = await open({
      multiple: false,
      filters: await filtersFor(filters),
    })
    return typeof chosen === 'string' ? chosen : null
  }

  /** Any number of files; empty when the dialog was cancelled. */
  async function pickManyAsync(
    filters?: DialogFilter[],
  ): Promise<PickedFile[]> {
    const chosen = await open({
      multiple: true,
      filters: await filtersFor(filters),
    })
    if (!chosen) return []
    return Array.isArray(chosen) ? chosen : [chosen]
  }

  /** Where to save, or `null` when the dialog was cancelled. */
  async function pickSaveTargetAsync(
    defaultName: string,
    filters?: DialogFilter[],
  ): Promise<PickedFile | null> {
    return await save({
      defaultPath: defaultName,
      filters: await filtersFor(filters),
    })
  }

  /** The name to show for a chosen file. */
  async function nameOfAsync(file: PickedFile): Promise<string> {
    return await invoke<string>('picked_file_name', { file })
  }

  return { pickOneAsync, pickManyAsync, pickSaveTargetAsync, nameOfAsync }
}
