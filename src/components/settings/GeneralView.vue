<script setup lang="ts">
/**
 * The category "Allgemein" (spec 023-settings-app, FR-005): device name and session restore.
 */
import { useSettingsDevice } from '~/components/settings/deviceContext'

const device = useSettingsDevice()
</script>

<template>
  <div class="flex flex-col gap-6">
    <SettingsAliasSetting
      :current-alias="device.info.value.alias ?? ''"
      @saved="device.reloadAsync"
    />
    <SettingsSessionRestoreSetting />
  </div>
</template>
