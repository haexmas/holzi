<script setup lang="ts">
import type { DeviceInfo } from '~/composables/useDevice'

// Settings-Screen for spec 002 US3+US4. Guarded by the `onboarded`
// middleware — an unset alias sends the operator back to the wizard,
// so we can assume `alias` is set here.
definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const { t } = useI18n()
const { currentDeviceInfoAsync } = useDevice()

const instanceName = computed(() => {
  const raw = route.params.instance
  return typeof raw === 'string' ? raw : Array.isArray(raw) ? (raw[0] ?? '') : ''
})

const backTarget = computed(() => `/workspace/${encodeURIComponent(instanceName.value)}`)

const deviceInfo = ref<DeviceInfo | null>(null)
const loadError = ref<string | null>(null)

async function reloadDeviceInfoAsync() {
  loadError.value = null
  try {
    deviceInfo.value = await currentDeviceInfoAsync()
  }
  catch (e) {
    loadError.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(reloadDeviceInfoAsync)
</script>

<template>
  <main class="min-h-screen flex flex-col p-6 gap-6">
    <header class="flex items-center justify-between gap-3 flex-wrap">
      <h1 class="text-2xl font-semibold">
        <template v-if="deviceInfo?.alias">
          {{ t('settings.header.forDevice', { alias: deviceInfo.alias }) }}
        </template>
        <template v-else>
          &nbsp;
        </template>
      </h1>
      <NuxtLink
        :to="backTarget"
        class="text-sm underline text-blue-600 hover:text-blue-800 focus:outline-none focus:ring-2 focus:ring-blue-500 rounded"
      >
        {{ t('workspace.heading', { instance: instanceName }) }}
      </NuxtLink>
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

      <hr class="border-neutral-200">

      <SettingsDefaultModelSetting
        v-if="deviceInfo"
        :device-uuid="deviceInfo.vaultDeviceUuid"
      />
    </div>
  </main>
</template>
