import { invoke } from '@tauri-apps/api/core'

export interface DeviceInfo {
  installationUuid: string
  vaultDeviceUuid: string
  /**
   * `null` before the onboarding wizard's final commit. The
   * `onboarded` middleware routes to `/onboarding/[instance]` while
   * this stays `null`.
   */
  alias: string | null
  /**
   * Raw OS hostname reported by `sysinfo`. `null` when the OS did not
   * return one; the frontend then falls back to
   * `$t('onboarding.alias.defaultPlaceholder')` as the initial input
   * value (spec 002 §FR-020 i18n boundary).
   */
  hostname: string | null
}

/**
 * Current-device identity for the active vault. Powers the onboarding
 * wizard's alias prefill, the workspace-landing header, and the
 * onboarded route-middleware.
 */
export function useDevice() {
  async function currentDeviceInfoAsync(): Promise<DeviceInfo> {
    return await invoke<DeviceInfo>('current_device_info')
  }

  async function updateDeviceAliasAsync(alias: string): Promise<void> {
    await invoke<void>('update_device_alias', { args: { alias } })
  }

  return { currentDeviceInfoAsync, updateDeviceAliasAsync }
}
