<script setup lang="ts">
/**
 * "Allgemein → Grundeinstellung" (spec 023-settings-app FR-005, spec 042): language, the row to
 * the vault-password view, device name and session restore.
 */
import { useSettingsDevice } from '~/components/settings/deviceContext'

const { t } = useI18n()
const device = useSettingsDevice()
</script>

<template>
  <div class="flex flex-col gap-6">
    <SettingsLanguageSetting />
    <SettingsGroup>
      <SettingsRow
        to="/general/basic/password"
        icon="lucide:key-round"
        :title="t('settings.locations.general.basic.password.title')"
        :description="
          t('settings.locations.general.basic.password.description')
        "
        data-testid="settings-row-general.basic.password"
      />
    </SettingsGroup>
    <SettingsAliasSetting
      :current-alias="device.info.value.alias ?? ''"
      @saved="device.reloadAsync"
    />
    <SettingsSessionRestoreSetting />
  </div>
</template>
