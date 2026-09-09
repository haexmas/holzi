import { invoke } from '@tauri-apps/api/core'

export type Fit = 'fits' | 'tight' | 'too_big' | 'unknown'

export interface CatalogEntry {
  id: string
  name: string
  family: string
  parameters: string
  quantization: string
  hf_repo: string
  hf_filename: string
  tokenizer_repo: string
  approx_size_bytes: number
  context_window: number
  license: string
}

export interface CatalogEntryWithFit extends CatalogEntry {
  fit: Fit
}

/**
 * Reads the built-in model catalog, annotated per entry with a fit
 * verdict against the current hardware. Serves the onboarding wizard's
 * "here are models that would work on this device" list.
 */
export function useCatalog() {
  async function listAsync(): Promise<CatalogEntryWithFit[]> {
    return await invoke<CatalogEntryWithFit[]>('list_catalog')
  }

  return { listAsync }
}
