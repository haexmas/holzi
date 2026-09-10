import { invoke } from '@tauri-apps/api/core'

export type ProviderKind = 'local' | 'api_key' | 'cli_delegate'

export interface Provider {
  id: string
  kind: ProviderKind
  adapter: string | null
  name: string
  baseUrl: string | null
  hasCredentials: boolean
  createdAt: number
}

export interface AddProviderArgs {
  kind: ProviderKind
  name: string
  adapter?: string
  baseUrl?: string
  apiKey?: string
}

/**
 * Result of an [`add_provider`] call. The provider is always inserted
 * on success; `refreshError` reports the auto-refresh outcome for
 * `api_key` providers — a non-null value means the row exists but
 * the model list could not be populated, so the frontend should toast
 * `refreshError` and let the operator retry [`refreshModelsAsync`].
 */
export interface AddProviderResult {
  provider: Provider
  modelCount: number | null
  refreshError: string | null
}

export interface ProviderModel {
  id: string
  name: string
  providerId: string
  contextWindow: number | null
}

export interface RefreshProviderModelsResult {
  providerId: string
  modelCount: number
  fetchedAt: number
}

/**
 * Provider CRUD + refresh. `addAsync` auto-refreshes the model list
 * for `api_key` providers as part of the same round trip; explicit
 * [`refreshModelsAsync`] is available for re-fetching later.
 */
export function useProviders() {
  /** Lists every provider configured for the active instance. */
  async function listAsync(): Promise<Provider[]> {
    return await invoke<Provider[]>('list_providers')
  }

  /** Adds a provider to the active instance. */
  async function addAsync(args: AddProviderArgs): Promise<AddProviderResult> {
    return await invoke<AddProviderResult>('add_provider', { args })
  }

  /** Deletes a provider from the active instance. */
  async function deleteAsync(id: string): Promise<void> {
    return await invoke<void>('delete_provider', { id })
  }

  /** Re-fetches the given provider's model list and replaces its cache. */
  async function refreshModelsAsync(
    providerId: string,
  ): Promise<RefreshProviderModelsResult> {
    return await invoke<RefreshProviderModelsResult>('refresh_provider_models', {
      providerId,
    })
  }

  /** Reads the cached models for one provider. Does not trigger a fetch. */
  async function listModelsAsync(providerId: string): Promise<ProviderModel[]> {
    return await invoke<ProviderModel[]>('list_provider_models', { providerId })
  }

  return { listAsync, addAsync, deleteAsync, refreshModelsAsync, listModelsAsync }
}
