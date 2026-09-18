<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { SttCatalogEntry } from '~/composables/useSttCatalog'

const { t } = useI18n()
const { errString } = useErrorString()
const { getPrefAsync, setPrefAsync } = usePreferences()
const { listAsync: listSttCatalogAsync } = useSttCatalog()
const { listInstalledAsync, downloadFromCatalogAsync } = useSttModels()

const props = defineProps<{
  deviceUuid: string
}>()

const PREF_KEY = 'voice.stt_model_id'
const DEFAULT_ID = 'whisper-tiny'

const catalog = ref<SttCatalogEntry[]>([])
const installedIds = ref<Set<string>>(new Set())
const activeId = ref(DEFAULT_ID)
const selectedId = ref(DEFAULT_ID)

const loading = ref(true)
const busy = ref(false)
const savedFlash = ref(false)
const opError = ref<string | null>(null)
const loadError = ref<string | null>(null)

function modelDisplayName(id: string): string {
  return catalog.value.find((e) => e.id === id)?.name ?? id
}

async function reloadAsync() {
  loading.value = true
  loadError.value = null
  try {
    const [entries, installed, pref] = await Promise.all([
      listSttCatalogAsync(),
      listInstalledAsync(),
      getPrefAsync({ kind: 'device', uuid: props.deviceUuid }, PREF_KEY),
    ])
    catalog.value = entries
    installedIds.value = new Set(installed.map((m) => m.id))
    const normalizedActiveId =
      pref && entries.some((entry) => entry.id === pref) ? pref : DEFAULT_ID
    activeId.value = normalizedActiveId
    selectedId.value = activeId.value
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
}

async function onSwitch() {
  if (!selectedId.value || selectedId.value === activeId.value) return
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    const installed = await downloadFromCatalogAsync(selectedId.value)
    await setPrefAsync(
      { kind: 'device', uuid: props.deviceUuid },
      PREF_KEY,
      installed.id,
    )
    await invoke('invalidate_stt_model_cache')
    activeId.value = installed.id
    installedIds.value = new Set([...installedIds.value, installed.id])
    savedFlash.value = true
  } catch (e) {
    opError.value = errString(e)
  } finally {
    busy.value = false
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-3">
    <h2 class="text-xl font-semibold">
      {{ t('settings.sttModel.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.sttModel.description') }}
    </p>

    <div v-if="loading" class="text-sm text-neutral-500">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <template v-if="!loading">
      <div class="text-sm">
        {{ t('settings.sttModel.current') }}:
        <strong>{{ modelDisplayName(activeId) }}</strong>
      </div>

      <label class="flex flex-col gap-1">
        <span class="text-sm font-medium">{{
          t('settings.sttModel.modelLabel')
        }}</span>
        <select
          v-model="selectedId"
          class="border border-neutral-300 rounded-md p-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          :disabled="busy"
        >
          <option v-for="entry in catalog" :key="entry.id" :value="entry.id">
            {{ entry.name }}{{ installedIds.has(entry.id) ? '' : ' …' }}
          </option>
        </select>
      </label>

      <div class="flex items-center gap-3 flex-wrap">
        <UiButton
          type="button"
          :disabled="busy || !selectedId || selectedId === activeId"
          @click="onSwitch"
        >
          {{
            busy
              ? t('settings.sttModel.downloading')
              : t('settings.sttModel.save')
          }}
        </UiButton>
        <span v-if="savedFlash" class="text-xs text-green-600" role="status">
          {{ t('settings.sttModel.saved') }}
        </span>
        <span v-if="opError" class="text-xs text-red-500" role="alert">
          {{ t('settings.sttModel.downloadFailed') }}: {{ opError }}
        </span>
      </div>
    </template>
  </section>
</template>
