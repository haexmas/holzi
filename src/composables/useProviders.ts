import { invoke } from '@tauri-apps/api/core'

export type ProviderKind = 'local' | 'api_key' | 'cli_delegate'

export interface Provider {
  id: string
  kind: ProviderKind
  name: string
  baseUrl: string | null
  hasCredentials: boolean
  createdAt: number
}

export interface AddProviderArgs {
  kind: ProviderKind
  name: string
  baseUrl?: string
  apiKey?: string
}

/**
 * Provider CRUD. This slice ships schema + storage; live provider
 * invocation (Anthropic/OpenAI HTTP, `cli_delegate` subprocess) is a
 * later slice.
 */
export function useProviders() {
  async function listAsync(): Promise<Provider[]> {
    return await invoke<Provider[]>('list_providers')
  }

  async function addAsync(args: AddProviderArgs): Promise<Provider> {
    return await invoke<Provider>('add_provider', { args })
  }

  async function deleteAsync(id: string): Promise<void> {
    return await invoke<void>('delete_provider', { id })
  }

  return { listAsync, addAsync, deleteAsync }
}
