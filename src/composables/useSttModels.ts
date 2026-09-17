import { makeInstalledModelsComposable } from '~/composables/useModels'

export interface InstalledSttModel {
  id: string
  name: string
}

/**
 * Lists installed local speech-to-text model tiers and downloads one from
 * the built-in catalog (spec 010). Shares its implementation with
 * `useModels` (chat/agent models) via `makeInstalledModelsComposable` —
 * only the Tauri command names and entry type differ.
 */
export const useSttModels = makeInstalledModelsComposable<InstalledSttModel>(
  'list_installed_stt_models',
  'download_stt_model',
)
