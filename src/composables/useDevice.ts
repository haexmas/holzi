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

/** One device of the vault in the settings' device list (spec 023-settings-app, FR-022). */
export interface VaultDevice {
  vaultDeviceUuid: string
  /** `null` for a device that has not finished onboarding. */
  alias: string | null
  isCurrent: boolean
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
    await invoke('update_device_alias', { args: { alias } })
  }

  /** This device first, then the others by name, unnamed ones last (ordered by the backend). */
  async function listVaultDevicesAsync(): Promise<VaultDevice[]> {
    return await invoke<VaultDevice[]>('list_vault_devices')
  }

  return {
    currentDeviceInfoAsync,
    updateDeviceAliasAsync,
    listVaultDevicesAsync,
  }
}
