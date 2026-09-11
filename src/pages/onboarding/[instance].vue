<script setup lang="ts">
import type { DeviceInfo } from '~/composables/useDevice'
import type { TierRecommendation } from '~/composables/useCatalog'

// Onboarding wizard for spec 002 US1. The middleware only sends us
// here while `alias` is still `null` on this device. We keep the
// entered alias local until the second step commits (either a model
// download finishes OR the operator chose "later via provider"); only
// then do we persist the alias and route to /workspace/[instance]. An
// exit between steps therefore leaves the vault in "still onboarding"
// state and the wizard re-opens next launch.
const route = useRoute()
const { t } = useI18n()
const { currentDeviceInfoAsync, updateDeviceAliasAsync } = useDevice()
const { recommendTiersAsync } = useCatalog()
const { downloadFromCatalogAsync } = useModels()
const { setPrefAsync } = usePreferences()

const instanceName = computed(() => {
  const raw = route.params.instance
  return typeof raw === 'string' ? raw : Array.isArray(raw) ? (raw[0] ?? '') : ''
})

const step = ref<'alias' | 'model'>('alias')
const deviceInfo = ref<DeviceInfo | null>(null)
const alias = ref('')
const tiers = ref<TierRecommendation[]>([])
const downloadingId = ref<string | null>(null)
const downloadError = ref<string | null>(null)
const loadError = ref<string | null>(null)

onMounted(async () => {
  try {
    deviceInfo.value = await currentDeviceInfoAsync()
    alias.value = deviceInfo.value?.alias ?? deviceInfo.value?.hostname ?? t('onboarding.alias.defaultPlaceholder')
  }
  catch (e) {
    loadError.value = e instanceof Error ? e.message : String(e)
    return
  }
  try {
    tiers.value = await recommendTiersAsync()
  }
  catch (e) {
    // Empty catalog only — the model step still lets the operator skip.
    tiers.value = []
    downloadError.value = e instanceof Error ? e.message : String(e)
  }
})

async function finalizeAliasAsync() {
  const trimmed = alias.value.trim()
  if (!trimmed) {
    throw new Error(t('onboarding.alias.required'))
  }
  await updateDeviceAliasAsync(trimmed)
}

async function completeWithoutModel() {
  try {
    await finalizeAliasAsync()
  }
  catch (e) {
    loadError.value = e instanceof Error ? e.message : String(e)
    return
  }
  await navigateTo(`/workspace/${encodeURIComponent(instanceName.value)}`)
}

async function completeWithModel(rec: TierRecommendation) {
  const info = deviceInfo.value
  if (!info) {
    return
  }
  downloadingId.value = rec.entry.id
  downloadError.value = null
  try {
    const installed = await downloadFromCatalogAsync(rec.entry.id)
    // Set device-scoped default model — the wizard's explicit
    // selection is a device standard (spec 002 §FR-007).
    await setPrefAsync(
      { kind: 'device', uuid: info.vaultDeviceUuid },
      'chat.default_model_id',
      installed.id,
    )
    await finalizeAliasAsync()
    await navigateTo(`/workspace/${encodeURIComponent(instanceName.value)}`)
  }
  catch (e) {
    downloadError.value = e instanceof Error ? e.message : String(e)
  }
  finally {
    downloadingId.value = null
  }
}
</script>

<template>
  <main class="min-h-screen flex flex-col items-center justify-center gap-6 p-6">
    <div class="w-full max-w-xl flex flex-col gap-4">
      <div>
        <h1 class="text-2xl font-semibold">
          {{ t('onboarding.wizard.title') }}
        </h1>
        <p class="text-sm text-neutral-500">
          {{ t('onboarding.wizard.subtitle') }}
        </p>
      </div>

      <p v-if="loadError" class="text-sm text-red-500" role="alert">
        {{ t('errors.deviceInfoFailed') }}: {{ loadError }}
      </p>

      <div v-if="!deviceInfo && !loadError" class="text-sm text-neutral-500">
        {{ t('onboarding.wizard.loadingDeviceInfo') }}
      </div>

      <template v-else-if="deviceInfo">
        <OnboardingAliasStep
          v-if="step === 'alias'"
          v-model="alias"
          :hostname-hint="deviceInfo.hostname"
          @next="step = 'model'"
        />
        <OnboardingModelChoiceStep
          v-else
          :tiers="tiers"
          :downloading-id="downloadingId"
          :download-error="downloadError"
          @choose="completeWithModel"
          @skip="completeWithoutModel"
          @back="step = 'alias'"
        />
      </template>
    </div>
  </main>
</template>
