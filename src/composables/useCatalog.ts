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

export type Tier = 'easy' | 'sweet' | 'max'

export interface TierRecommendation {
  tier: Tier
  entry: CatalogEntry
  fit: Fit
}

/**
 * Reads the built-in model catalog, annotated per entry with a fit
 * verdict against the current hardware. Serves the onboarding wizard's
 * "here are models that would work on this device" list.
 */
export function useCatalog() {
  /** Returns every bundled catalog entry with its hardware-fit verdict. */
  async function listAsync(): Promise<CatalogEntryWithFit[]> {
    return await invoke<CatalogEntryWithFit[]>('list_catalog')
  }

  /**
   * Returns exactly three tier-labelled recommendations
   * (Easy/Sweet/Max) for the onboarding wizard. Backend implements
   * the fallback rules from spec 002 §catalog_recommend_tiers.
   */
  async function recommendTiersAsync(): Promise<TierRecommendation[]> {
    return await invoke<TierRecommendation[]>('catalog_recommend_tiers')
  }

  return { listAsync, recommendTiersAsync }
}
