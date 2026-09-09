import { invoke } from '@tauri-apps/api/core'
import type { InstanceInfo } from '@bindings/InstanceInfo'
import type { CreateInstanceArgs } from '@bindings/CreateInstanceArgs'
import type { CreateInstanceResult } from '@bindings/CreateInstanceResult'
import type { OpenInstanceArgs } from '@bindings/OpenInstanceArgs'

/**
 * Thin wrapper around the Tauri command surface. Each function is a
 * one-shot `invoke()` that re-throws typed HolziError on failure.
 */
export function useInstance() {
  async function listAsync(): Promise<InstanceInfo[]> {
    return await invoke<InstanceInfo[]>('list_instances')
  }

  async function createAsync(args: CreateInstanceArgs): Promise<CreateInstanceResult> {
    return await invoke<CreateInstanceResult>('create_instance', { args })
  }

  async function openAsync(args: OpenInstanceArgs): Promise<InstanceInfo> {
    return await invoke<InstanceInfo>('open_instance', { args })
  }

  async function closeAsync(): Promise<void> {
    return await invoke<void>('close_instance')
  }

  return { listAsync, createAsync, openAsync, closeAsync }
}
