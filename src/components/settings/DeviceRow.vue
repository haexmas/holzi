<script setup lang="ts">
/**
 * One device of the vault (spec 024, user story 4, FR-033 to FR-035): its name, its role, this device
 * marked, and for the others whether it is online, else when it was last online, or "never seen";
 * a device sync is halted with shows why. `now` comes from the list, so all rows age together.
 */
import type { VaultDevice } from '@bindings/VaultDevice'
import {
  deviceStatus,
  elapsedSince,
  problemLabelKey,
  roleLabelKey,
} from '~/lib/sync/deviceStatus'

const props = defineProps<{
  device: VaultDevice
  now: number
  /** A main device may remove any other device (FR-035); never this one (FR-026). */
  removable?: boolean
}>()

const { t } = useI18n()
const router = useTabRouter()

function onRemove() {
  router.push(`/federation/devices/${props.device.devicePubkey}/remove`)
}

const status = computed(() => {
  const state = deviceStatus(props.device)
  if (state.kind === 'online') return t('settings.federation.status.online')
  if (state.kind === 'neverSeen') {
    return t('settings.federation.status.neverSeen')
  }
  const elapsed = elapsedSince(state.at, props.now)
  return elapsed.unit === 'justNow'
    ? t('settings.federation.status.justNow')
    : t(`settings.federation.status.${elapsed.unit}`, { n: elapsed.value })
})
</script>

<template>
  <SettingsRow
    :title="device.alias ?? t('settings.federation.unnamed')"
    :icon="device.isCurrent ? 'lucide:monitor-smartphone' : 'lucide:monitor'"
    data-testid="settings-device"
    :data-current="device.isCurrent || undefined"
    :data-online="device.online || undefined"
    :data-role="device.role"
  >
    <template #description>
      <span data-testid="settings-device-role">
        {{ t(roleLabelKey(device.role)) }}
      </span>
      <template v-if="!device.isCurrent">
        ·
        <span data-testid="settings-device-status">{{ status }}</span>
      </template>
      <span
        v-if="device.problem"
        class="block text-destructive"
        data-testid="settings-device-problem"
        role="alert"
      >
        {{ t(problemLabelKey(device.problem)) }}
      </span>
    </template>
    <span
      v-if="device.isCurrent"
      data-testid="settings-device-current"
      class="rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary"
    >
      {{ t('settings.federation.thisDevice') }}
    </span>
    <span
      v-else-if="device.online"
      class="rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary"
    >
      {{ t('settings.federation.status.online') }}
    </span>
    <UiButton
      v-if="removable && !device.isCurrent"
      variant="outline"
      data-testid="settings-device-remove"
      @click="onRemove"
    >
      {{ t('settings.remove.button') }}
    </UiButton>
  </SettingsRow>
</template>
