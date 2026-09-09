import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface InstalledModel {
  id: string
  name: string
  providerId: string
  contextWindow: number | null
  relativePath: string
  sizeBytes: number
}

export interface DownloadProgressEvent {
  modelId: string
  bytesDownloaded: number
  bytesTotal: number | null
}

export interface DownloadFromHfArgs {
  id: string
  name: string
  hfRepo: string
  hfFilename: string
  tokenizerRepo: string
  contextWindow?: number | null
}

export interface ImportModelArgs {
  id: string
  name: string
  sourcePath: string
  tokenizerRepo: string
  contextWindow?: number | null
  filename?: string
}

/**
 * Thin wrapper around the model management commands and the two
 * progress-related Tauri events (`model-download-progress`,
 * `model-download-complete`).
 */
export function useModels() {
  async function listInstalledAsync(): Promise<InstalledModel[]> {
    return await invoke<InstalledModel[]>('list_installed_models')
  }

  async function downloadFromCatalogAsync(catalogId: string): Promise<InstalledModel> {
    return await invoke<InstalledModel>('download_model_from_catalog', { catalogId })
  }

  async function downloadFromHfAsync(args: DownloadFromHfArgs): Promise<InstalledModel> {
    return await invoke<InstalledModel>('download_model_from_hf', { args })
  }

  async function importFromFileAsync(args: ImportModelArgs): Promise<InstalledModel> {
    return await invoke<InstalledModel>('import_model_from_file', { args })
  }

  async function deleteAsync(id: string): Promise<void> {
    return await invoke<void>('delete_installed_model', { id })
  }

  /**
   * Subscribes to per-chunk progress events. Returns the unlisten
   * function — callers MUST call it on component unmount to avoid a
   * leaked listener.
   */
  async function onDownloadProgress(
    handler: (event: DownloadProgressEvent) => void,
  ): Promise<UnlistenFn> {
    return await listen<DownloadProgressEvent>('model-download-progress', (e) => {
      handler(e.payload)
    })
  }

  async function onDownloadComplete(
    handler: (model: InstalledModel) => void,
  ): Promise<UnlistenFn> {
    return await listen<InstalledModel>('model-download-complete', (e) => {
      handler(e.payload)
    })
  }

  return {
    listInstalledAsync,
    downloadFromCatalogAsync,
    downloadFromHfAsync,
    importFromFileAsync,
    deleteAsync,
    onDownloadProgress,
    onDownloadComplete,
  }
}
