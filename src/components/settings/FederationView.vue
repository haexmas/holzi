<script setup lang="ts">
/**
 * The category "Föderation" (spec 023-settings-app, FR-022, research R12): the vault's devices,
 * this one first and marked, the others by name, unnamed ones last (ordered by the backend). A
 * display only: the own name is changed in "Allgemein", and "last online" waits for the sync spec.
 * Loaded when the category opens, so a renamed device shows its new name the next time.
 */
import type { VaultDevice } from '~/composables/useDevice'

const { t } = useI18n()
const { errString } = useErrorString()
const { listVaultDevicesAsync } = useDevice()

const devices = ref<VaultDevice[]>([])
const loading = ref(true)
const loadError = ref<string | null>(null)

onMounted(async () => {
  try {
    devices.value = await listVaultDevicesAsync()
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <section class="flex flex-col gap-2">
    <div v-if="loading" class="text-sm text-muted-foreground">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-destructive" role="alert">
      {{ t('settings.federation.loadFailed') }}: {{ loadError }}
    </p>

    <SettingsGroup>
      <SettingsRow
        to="/federation/devices/link"
        icon="lucide:qr-code"
        :title="t('settings.locations.federation.link.title')"
        :description="t('settings.locations.federation.link.description')"
        data-testid="settings-link-device"
      />
    </SettingsGroup>

    <SettingsGroup
      v-if="!loading && !loadError"
      :label="t('settings.federation.devicesLabel')"
    >
      <SettingsRow
        v-for="device in devices"
        :key="device.vaultDeviceUuid"
        :title="device.alias ?? t('settings.federation.unnamed')"
        :icon="
          device.isCurrent ? 'lucide:monitor-smartphone' : 'lucide:monitor'
        "
        data-testid="settings-device"
        :data-current="device.isCurrent || undefined"
      >
        <span
          v-if="device.isCurrent"
          data-testid="settings-device-current"
          class="rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary"
        >
          {{ t('settings.federation.thisDevice') }}
        </span>
      </SettingsRow>
    </SettingsGroup>
  </section>
</template>
