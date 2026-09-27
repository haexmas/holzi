import { inject, type InjectionKey, type Ref } from 'vue'
import type { DeviceInfo } from '~/composables/useDevice'

/**
 * This device's identity for the settings views (spec 023-settings-app, contracts §1). Route
 * components get no props, so `apps/SettingsApp.vue` loads the device once and provides it; it
 * renders its content only after the load, so `info` is set for every consumer.
 */
export type SettingsDevice = {
  info: Ref<DeviceInfo>
  reloadAsync(): Promise<void>
}

export const SETTINGS_DEVICE_KEY: InjectionKey<SettingsDevice> =
  Symbol('settingsDevice')

export function useSettingsDevice(): SettingsDevice {
  const device = inject(SETTINGS_DEVICE_KEY)
  if (!device) throw new Error('settings view outside apps/SettingsApp.vue')
  return device
}
