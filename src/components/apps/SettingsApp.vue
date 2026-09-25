<script setup lang="ts">
import type { DeviceInfo } from '~/composables/useDevice'

// Settings app for spec 002 US3+US4, moved into the window manager (spec
// 015-workspace-shell, T022). Onboarding enforcement (an unset alias sends
// the operator back to the wizard, so `alias` can be assumed set here) now
// lives on the window manager host page (T025) — this component owns no route of
// its own anymore, so there is no "back to workspace" link either: closing
// or switching away from this tab is the window manager's own affordance.
const { t } = useI18n()
const { errString } = useErrorString()
const { currentDeviceInfoAsync } = useDevice()

const deviceInfo = ref<DeviceInfo | null>(null)
const loadError = ref<string | null>(null)

async function reloadDeviceInfoAsync() {
  loadError.value = null
  try {
    deviceInfo.value = await currentDeviceInfoAsync()
  } catch (e) {
    loadError.value = errString(e)
  }
}

onMounted(reloadDeviceInfoAsync)
</script>

<template>
  <main class="h-full min-h-0 overflow-y-auto flex flex-col p-6 gap-6">
    <header class="flex items-center justify-between gap-3 flex-wrap">
      <h1 class="text-2xl font-semibold">
        <template v-if="deviceInfo?.alias">
          {{ t('settings.header.forDevice', { alias: deviceInfo.alias }) }}
        </template>
        <template v-else> &nbsp; </template>
      </h1>
    </header>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('errors.deviceInfoFailed') }}: {{ loadError }}
    </p>

    <div class="flex flex-col gap-8 max-w-2xl">
      <SettingsAliasSetting
        v-if="deviceInfo?.alias"
        :current-alias="deviceInfo.alias"
        @saved="reloadDeviceInfoAsync"
      />

      <hr class="border-neutral-200" />

      <SettingsDefaultModelSetting
        v-if="deviceInfo"
        :device-uuid="deviceInfo.vaultDeviceUuid"
      />

      <hr class="border-neutral-200" />

      <SettingsSttModelSetting
        v-if="deviceInfo"
        :device-uuid="deviceInfo.vaultDeviceUuid"
      />

      <hr class="border-neutral-200" />

      <ModelsHuggingFaceModelManagement />

      <hr class="border-neutral-200" />

      <SettingsConnectDelegateProvider />

      <hr class="border-neutral-200" />

      <SettingsAutonomyModeSetting
        v-if="deviceInfo"
        :device-uuid="deviceInfo.vaultDeviceUuid"
      />

      <hr class="border-neutral-200" />

      <SettingsDelegateDenyRulesSetting
        v-if="deviceInfo"
        :device-uuid="deviceInfo.vaultDeviceUuid"
      />
    </div>
  </main>
</template>
