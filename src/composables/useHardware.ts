import { invoke } from '@tauri-apps/api/core'

export type Backend = 'cpu' | 'cuda' | 'metal'

export interface HardwareInfo {
  backend: Backend
  total_ram_bytes: number
  available_ram_bytes: number
  vram_bytes: number | null
}

/**
 * Reads the current hardware snapshot from the Rust side. Cheap enough
 * to call per catalog-listing refresh; the CUDA VRAM probe is capped
 * at 500 ms wall clock server-side.
 */
export function useHardware() {
  async function getInfoAsync(): Promise<HardwareInfo> {
    return await invoke<HardwareInfo>('get_hardware_info')
  }

  return { getInfoAsync }
}
