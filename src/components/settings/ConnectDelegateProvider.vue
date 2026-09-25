<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { DelegateVendor, Provider } from '~/composables/useProviders'

const { t } = useI18n()
const { errString } = useErrorString()
const { listAsync, onDelegateConnectProgress } = useProviders()
// Spec 020 FR-024: provider writes run catalog actions (connect/submit are guardrails).
const refreshModels = useActionOrThrow('settings.delegate.refreshModels')
const connect = useActionOrThrow('settings.delegate.connectProvider')
const submitCode = useActionOrThrow('settings.delegate.submitCode')

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
// "Refresh models" (spec 012 FR-022): re-fetches a connected provider's
// models and their capabilities without a new sign-in.
const refreshing = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})
const refreshed = reactive<Record<DelegateVendor, boolean>>({
  claude: false,
  codex: false,
})
const refreshError = reactive<Record<DelegateVendor, string | null>>({
  claude: null,
  codex: null,
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
  refreshed[vendor] = false
  refreshError[vendor] = null
}

/**
 * Re-fetches the connected provider's models. The chat page re-reads its
 * model lists on mount, so this needs no store coupling; on failure the
 * previously stored capabilities stay untouched (the backend keeps the old
 * cache when a refresh fails).
 */
async function onRefreshModels(vendor: DelegateVendor) {
  const provider = connectedProvider(vendor)
  if (!provider) return
  refreshed[vendor] = false
  refreshError[vendor] = null
  refreshing[vendor] = true
  try {
    await refreshModels({ providerId: provider.id })
    refreshed[vendor] = true
  } catch (e) {
    refreshError[vendor] = errString(e)
  } finally {
    refreshing[vendor] = false
  }
}

async function onConnect(vendor: DelegateVendor) {
  resetVendorState(vendor)
  connecting[vendor] = true
  try {
    const result = (await connect({ vendor, name: vendorLabel(vendor) })) as {
      status: string
    }
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
    await submitCode({ code: codeInput[vendor], name: vendorLabel(vendor) })
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
              :disabled="refreshing[vendor]"
              @click="onRefreshModels(vendor)"
            >
              {{
                refreshing[vendor]
                  ? t('settings.cliDelegate.refreshing')
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
        <span
          v-if="refreshed[vendor]"
          class="text-xs text-green-600"
          role="status"
        >
          {{ t('settings.cliDelegate.refreshed') }}
        </span>
        <span
          v-if="refreshError[vendor]"
          class="text-xs text-red-500"
          role="alert"
        >
          {{ t('settings.cliDelegate.refreshFailed') }}:
          {{ refreshError[vendor] }}
        </span>
        <span v-if="opError[vendor]" class="text-xs text-red-500" role="alert">
          {{ t('settings.cliDelegate.error') }}: {{ opError[vendor] }}
        </span>
      </div>
    </div>
  </section>
</template>
