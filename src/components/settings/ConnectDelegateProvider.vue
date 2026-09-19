<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { DelegateVendor, Provider } from '~/composables/useProviders'

const { t } = useI18n()
const { errString } = useErrorString()
const {
  listAsync,
  connectCliDelegateAsync,
  submitCliDelegateCodeAsync,
  refreshModelsAsync,
  onDelegateConnectProgress,
} = useProviders()

const VENDORS: DelegateVendor[] = ['claude', 'codex']

const providerList = ref<Provider[]>([])
const loading = ref(true)
const loadError = ref<string | null>(null)

const connecting = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})
const awaitingCode = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})
const progressUrl = reactive<Record<DelegateVendor, string | null>>({
  claude: null,
  codex: null,
})
const progressCode = reactive<Record<DelegateVendor, string | null>>({
  claude: null,
  codex: null,
})
const codeInput = reactive<Record<DelegateVendor, string>>({
  claude: '',
  codex: '',
})
const opError = reactive<Record<DelegateVendor, string | null>>({
  claude: null,
  codex: null,
})
const successFlash = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})
const refreshingModels = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})
const modelsRefreshError = reactive<Record<DelegateVendor, string | null>>({
  claude: null,
  codex: null,
})
const modelsRefreshSuccess = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})

let unlisten: UnlistenFn | null = null

function vendorLabel(vendor: DelegateVendor): string {
  return t(`chat.model.delegate.${vendor}`)
}

function connectedProvider(vendor: DelegateVendor): Provider | undefined {
  return providerList.value.find(
    (p) => p.kind === 'cli_delegate' && p.adapter === vendor,
  )
}

async function reloadAsync() {
  loading.value = true
  loadError.value = null
  try {
    providerList.value = await listAsync()
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
}

function resetVendorState(vendor: DelegateVendor) {
  awaitingCode[vendor] = false
  progressUrl[vendor] = null
  progressCode[vendor] = null
  codeInput[vendor] = ''
  opError[vendor] = null
  successFlash[vendor] = false
}

async function onConnect(vendor: DelegateVendor) {
  resetVendorState(vendor)
  connecting[vendor] = true
  try {
    const result = await connectCliDelegateAsync({
      vendor,
      name: vendorLabel(vendor),
    })
    if (result.status === 'awaiting_code') {
      awaitingCode[vendor] = true
    } else {
      await reloadAsync()
      successFlash[vendor] = true
      connecting[vendor] = false
    }
  } catch (e) {
    opError[vendor] = errString(e)
    connecting[vendor] = false
  }
}

async function onSubmitCode(vendor: DelegateVendor) {
  if (!codeInput[vendor].trim()) return
  opError[vendor] = null
  try {
    await submitCliDelegateCodeAsync({
      code: codeInput[vendor],
      name: vendorLabel(vendor),
    })
    await reloadAsync()
    awaitingCode[vendor] = false
    successFlash[vendor] = true
    connecting[vendor] = false
  } catch (e) {
    // The backend keeps the flow open on failure (a rejected code commonly
    // re-prompts rather than exiting) — stay in the code-entry state so the
    // operator can retry without restarting the whole connect flow.
    opError[vendor] = errString(e)
  }
}

/**
 * Re-fetches the connected provider's live model catalog (spec: Claude's
 * comes from Anthropic's `/v1/models`). Separate from `onConnect` — this
 * needs no OAuth round trip, just `refresh_provider_models` against the
 * already-stored credentials.
 */
async function onRefreshModels(vendor: DelegateVendor) {
  const provider = connectedProvider(vendor)
  if (!provider) return
  modelsRefreshError[vendor] = null
  modelsRefreshSuccess[vendor] = false
  refreshingModels[vendor] = true
  try {
    await refreshModelsAsync(provider.id)
    modelsRefreshSuccess[vendor] = true
  } catch (e) {
    modelsRefreshError[vendor] = errString(e)
  } finally {
    refreshingModels[vendor] = false
  }
}

onMounted(async () => {
  await reloadAsync()
  unlisten = await onDelegateConnectProgress(async (event) => {
    if (
      event.status === 'awaiting_browser' ||
      event.status === 'awaiting_code'
    ) {
      progressUrl[event.vendor] = event.url ?? null
      progressCode[event.vendor] = event.code ?? null
    } else if (event.status === 'success') {
      // Claude can complete on its own once the browser round-trip
      // finishes, without any submitCliDelegateCodeAsync call ever
      // resolving — this event is the only signal for that case.
      awaitingCode[event.vendor] = false
      connecting[event.vendor] = false
      await reloadAsync()
      successFlash[event.vendor] = true
    } else if (event.status === 'error') {
      awaitingCode[event.vendor] = false
      opError[event.vendor] = event.message ?? null
      connecting[event.vendor] = false
    }
  })
})

onBeforeUnmount(() => {
  unlisten?.()
})
</script>

<template>
  <section class="flex flex-col gap-3">
    <h2 class="text-xl font-semibold">
      {{ t('settings.cliDelegate.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.cliDelegate.description') }}
    </p>

    <div v-if="loading" class="text-sm text-neutral-500">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <div v-if="!loading" class="flex flex-col gap-4">
      <div
        v-for="vendor in VENDORS"
        :key="vendor"
        class="flex flex-col gap-2 border border-neutral-200 rounded-md p-3"
      >
        <div class="flex items-center justify-between gap-3 flex-wrap">
          <span class="text-sm font-medium">{{ vendorLabel(vendor) }}</span>
          <span
            v-if="connectedProvider(vendor) && !awaitingCode[vendor]"
            class="text-xs text-green-600"
          >
            {{ t('settings.cliDelegate.connected') }}
          </span>
        </div>

        <template v-if="awaitingCode[vendor]">
          <p class="text-sm text-neutral-500">
            {{ t('settings.cliDelegate.awaitingAuto') }}
          </p>
          <p class="text-sm">
            {{ t('settings.cliDelegate.openUrlPrompt') }}
          </p>
          <a
            v-if="progressUrl[vendor]"
            :href="progressUrl[vendor]!"
            class="text-sm underline text-blue-600 hover:text-blue-800 break-all"
            @click.prevent="openUrl(progressUrl[vendor]!)"
          >
            {{ progressUrl[vendor] }}
          </a>
          <label class="flex flex-col gap-1">
            <span class="text-sm font-medium">
              {{ t('settings.cliDelegate.codeInputLabel') }}
            </span>
            <input
              v-model="codeInput[vendor]"
              type="text"
              autocomplete="off"
              class="border border-neutral-300 rounded-md p-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              @keyup.enter="onSubmitCode(vendor)"
            />
          </label>
          <div class="flex items-center gap-3 flex-wrap">
            <UiButton
              type="button"
              :disabled="!codeInput[vendor].trim()"
              @click="onSubmitCode(vendor)"
            >
              {{ t('settings.cliDelegate.submitCode') }}
            </UiButton>
          </div>
        </template>

        <template v-else-if="connecting[vendor]">
          <p class="text-sm text-neutral-500">
            {{ t('settings.cliDelegate.connecting') }}
          </p>
          <template v-if="progressUrl[vendor]">
            <a
              :href="progressUrl[vendor]!"
              class="text-sm underline text-blue-600 hover:text-blue-800 break-all"
              @click.prevent="openUrl(progressUrl[vendor]!)"
            >
              {{ progressUrl[vendor] }}
            </a>
            <p v-if="progressCode[vendor]" class="text-sm">
              {{ t('settings.cliDelegate.enterCodePrompt') }}
              <strong>{{ progressCode[vendor] }}</strong>
            </p>
          </template>
        </template>

        <template v-else>
          <div class="flex items-center gap-3 flex-wrap">
            <UiButton
              type="button"
              variant="outline"
              @click="onConnect(vendor)"
            >
              {{
                connectedProvider(vendor)
                  ? t('settings.cliDelegate.reconnect')
                  : t('settings.cliDelegate.connect')
              }}
            </UiButton>
            <UiButton
              v-if="connectedProvider(vendor)"
              type="button"
              variant="outline"
              :disabled="refreshingModels[vendor]"
              @click="onRefreshModels(vendor)"
            >
              {{
                refreshingModels[vendor]
                  ? t('settings.cliDelegate.refreshingModels')
                  : t('settings.cliDelegate.refreshModels')
              }}
            </UiButton>
          </div>
        </template>

        <span
          v-if="successFlash[vendor]"
          class="text-xs text-green-600"
          role="status"
        >
          {{ t('settings.cliDelegate.success') }}
        </span>
        <span v-if="opError[vendor]" class="text-xs text-red-500" role="alert">
          {{ t('settings.cliDelegate.error') }}: {{ opError[vendor] }}
        </span>
        <span
          v-if="modelsRefreshSuccess[vendor]"
          class="text-xs text-green-600"
          role="status"
        >
          {{ t('settings.cliDelegate.modelsRefreshed') }}
        </span>
        <span
          v-if="modelsRefreshError[vendor]"
          class="text-xs text-red-500"
          role="alert"
        >
          {{ t('settings.cliDelegate.refreshModelsError') }}:
          {{ modelsRefreshError[vendor] }}
        </span>
      </div>
    </div>
  </section>
</template>
