<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import { hfErrorKey, useHuggingFace, type HuggingFaceModelResult, type HuggingFaceUpdateStatus } from '~/composables/useHuggingFace'
import { useModels, type DownloadProgressEvent, type InstalledModel } from '~/composables/useModels'
import { useCatalog, type CatalogEntryWithFit } from '~/composables/useCatalog'
import { useChat } from '~/composables/useChat'

const { t } = useI18n()
const {
  listInstalledAsync,
  deleteAsync,
  downloadFromCatalogAsync,
  downloadFromHfAsync,
  onDownloadProgress,
  onDownloadComplete,
} = useModels()
const { checkUpdatesAsync, installUpdateAsync } = useHuggingFace()
const { listAsync: listCatalogAsync } = useCatalog()
const { loadModelAsync, loadModelWithIntegrityOverrideAsync, activeModelInfoAsync } = useChat()

type Tab = 'catalog' | 'search' | 'installed'
const activeTab = ref<Tab>('installed')

const installed = ref<InstalledModel[]>([])
const catalogEntries = ref<CatalogEntryWithFit[]>([])
const activeModelId = ref<string | null>(null)

const listErrorKey = ref<string | null>(null)
const loading = ref(true)

const selectedRepo = ref<HuggingFaceModelResult | null>(null)

const updateStatuses = ref<Record<string, HuggingFaceUpdateStatus>>({})
const checkingUpdates = ref(false)
const updateErrorKey = ref<string | null>(null)
const installingUpdateId = ref<string | null>(null)

const busyModelId = ref<string | null>(null)
const deleteErrorKey = ref<string | null>(null)
const loadErrorKey = ref<string | null>(null)
const downloadStates = ref<Record<string, DownloadProgressEvent>>({})

let unlistenDownloadProgress: UnlistenFn | null = null
let unlistenDownloadComplete: UnlistenFn | null = null

interface IntegrityDialogState {
  modelId: string
  errorKind: 'ModelIntegrityMismatch' | 'ModelIntegrityUnknown' | 'ModelIntegrityError'
  expected: string | null
  actual: string | null
}
const integrityDialog = ref<IntegrityDialogState | null>(null)
const integrityBusy = ref(false)
const integrityActionError = ref<string | null>(null)

async function reloadAsync() {
  loading.value = true
  listErrorKey.value = null
  try {
    const [installedList, catalogList, active] = await Promise.all([
      listInstalledAsync(),
      listCatalogAsync(),
      activeModelInfoAsync(),
    ])
    installed.value = installedList
    catalogEntries.value = catalogList
    activeModelId.value = active?.modelId ?? null
  }
  catch (e) {
    listErrorKey.value = hfErrorKey(e)
  }
  finally {
    loading.value = false
  }
}

async function checkUpdatesNowAsync() {
  checkingUpdates.value = true
  updateErrorKey.value = null
  try {
    const statuses = await checkUpdatesAsync()
    updateStatuses.value = Object.fromEntries(statuses.map((s) => [s.modelId, s]))
  }
  catch (e) {
    updateErrorKey.value = hfErrorKey(e)
  }
  finally {
    checkingUpdates.value = false
  }
}

async function installUpdateForAsync(modelId: string) {
  installingUpdateId.value = modelId
  updateErrorKey.value = null
  try {
    await installUpdateAsync(modelId)
    await reloadAsync()
    await checkUpdatesNowAsync()
  }
  catch (e) {
    updateErrorKey.value = hfErrorKey(e)
  }
  finally {
    installingUpdateId.value = null
    clearDownloadState(modelId)
  }
}

async function downloadCatalogEntryAsync(entry: CatalogEntryWithFit) {
  busyModelId.value = entry.id
  listErrorKey.value = null
  try {
    await downloadFromCatalogAsync(entry.id)
    await reloadAsync()
  }
  catch (e) {
    listErrorKey.value = hfErrorKey(e)
  }
  finally {
    busyModelId.value = null
    clearDownloadState(entry.id)
  }
}

function setDownloadState(event: DownloadProgressEvent) {
  downloadStates.value = {
    ...downloadStates.value,
    [event.modelId]: event,
  }
}

function clearDownloadState(modelId: string) {
  if (!downloadStates.value[modelId]) return
  const next = { ...downloadStates.value }
  delete next[modelId]
  downloadStates.value = next
}

function downloadPercent(modelId: string): number | null {
  const state = downloadStates.value[modelId]
  if (!state || state.bytesTotal === null || state.bytesTotal <= 0) return null
  return Math.min(100, Math.max(0, Math.round((state.bytesDownloaded / state.bytesTotal) * 100)))
}

function downloadWidth(modelId: string): string {
  return `${downloadPercent(modelId) ?? 35}%`
}

function humanBytes(n: number | null): string {
  if (n === null) return t('models.filePicker.sizeUnknown')
  const kb = 1024
  const mb = kb * 1024
  const gb = mb * 1024
  if (n >= gb) return `${(n / gb).toFixed(1)} GB`
  if (n >= mb) return `${(n / mb).toFixed(0)} MB`
  return `${(n / kb).toFixed(0)} KB`
}

async function deleteModelAsync(id: string) {
  // eslint-disable-next-line no-alert
  if (!confirm(t('models.installed.deleteConfirm'))) return
  busyModelId.value = id
  deleteErrorKey.value = null
  try {
    await deleteAsync(id)
    await reloadAsync()
  }
  catch (e) {
    deleteErrorKey.value = hfErrorKey(e)
  }
  finally {
    busyModelId.value = null
  }
}

function openIntegrityDialog(modelId: string, e: unknown) {
  const err = e as { kind?: string, expectedSha256?: string | null, actualSha256?: string | null }
  const kind = err.kind
  if (kind !== 'ModelIntegrityMismatch' && kind !== 'ModelIntegrityUnknown' && kind !== 'ModelIntegrityError') {
    return false
  }
  integrityDialog.value = {
    modelId,
    errorKind: kind,
    expected: err.expectedSha256 ?? null,
    actual: err.actualSha256 ?? null,
  }
  return true
}

async function loadModelHereAsync(id: string) {
  busyModelId.value = id
  loadErrorKey.value = null
  try {
    await loadModelAsync(id)
    activeModelId.value = id
  }
  catch (e) {
    if (!openIntegrityDialog(id, e)) {
      loadErrorKey.value = hfErrorKey(e)
    }
  }
  finally {
    busyModelId.value = null
  }
}

async function onLoadUntrustedAsync() {
  if (!integrityDialog.value) return
  integrityBusy.value = true
  integrityActionError.value = null
  try {
    await loadModelWithIntegrityOverrideAsync(integrityDialog.value.modelId)
    activeModelId.value = integrityDialog.value.modelId
    integrityDialog.value = null
    await reloadAsync()
  }
  catch (e) {
    integrityActionError.value = t(hfErrorKey(e))
  }
  finally {
    integrityBusy.value = false
  }
}

async function onRepairSourceAsync() {
  const dialog = integrityDialog.value
  if (!dialog) return
  const model = installed.value.find((m) => m.id === dialog.modelId)
  integrityBusy.value = true
  integrityActionError.value = null
  try {
    if (model?.sourceKind === 'huggingface' && model.hfRepo && model.hfFilename) {
      await downloadFromHfAsync({
        repoId: model.hfRepo,
        filename: model.hfFilename,
        revision: model.hfRevisionRef ?? undefined,
        name: model.name,
        contextWindow: model.contextWindow,
      })
      integrityDialog.value = null
      await reloadAsync()
    }
    else {
      // Imported models have no remote source to re-fetch automatically;
      // the operator must re-run the local file import.
      integrityActionError.value = t('models.integrityDialog.actionFailed')
    }
  }
  catch (e) {
    integrityActionError.value = t(hfErrorKey(e))
  }
  finally {
    integrityBusy.value = false
    clearDownloadState(dialog.modelId)
  }
}

function onChooseOther() {
  integrityDialog.value = null
  activeTab.value = 'installed'
}

function onIntegrityDialogOpenChange(open: boolean) {
  if (!open) {
    integrityDialog.value = null
    integrityActionError.value = null
  }
}

function onFilePickerInstalled(_model: InstalledModel) {
  clearDownloadState(_model.id)
  selectedRepo.value = null
  activeTab.value = 'installed'
  void reloadAsync()
}

onMounted(async () => {
  unlistenDownloadProgress = await onDownloadProgress(setDownloadState)
  unlistenDownloadComplete = await onDownloadComplete((model) => {
    const state = downloadStates.value[model.id]
    if (!state) return
    setDownloadState({
      modelId: model.id,
      bytesDownloaded: model.sizeBytes,
      bytesTotal: model.sizeBytes,
    })
  })
  await reloadAsync()
  // A management view visit is an explicit, user-visible update check. The
  // command is read-only and only checks models with a stored HF ref.
  await checkUpdatesNowAsync()
})

onBeforeUnmount(() => {
  unlistenDownloadProgress?.()
  unlistenDownloadComplete?.()
})
</script>

<template>
  <section class="flex flex-col gap-4">
    <h2 class="text-xl font-semibold">
      {{ t('models.management.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('models.management.description') }}
    </p>

    <div role="tablist" class="flex gap-2 border-b border-neutral-200">
      <button
        v-for="tab in (['installed', 'search', 'catalog'] as Tab[])"
        :key="tab"
        type="button"
        role="tab"
        :aria-selected="activeTab === tab"
        class="px-3 py-2 text-sm"
        :class="activeTab === tab ? 'border-b-2 border-blue-500 font-medium' : 'text-neutral-500'"
        @click="activeTab = tab; selectedRepo = null"
      >
        {{ t(`models.management.tabs.${tab}`) }}
      </button>
    </div>

    <p v-if="listErrorKey" class="text-sm text-red-500" role="alert">
      {{ t(listErrorKey) }}
    </p>

    <div v-if="loading" class="text-sm text-neutral-500">
      {{ t('models.search.loading') }}
    </div>

    <template v-else>
      <div v-if="activeTab === 'installed'" class="flex flex-col gap-2">
        <div class="flex items-center justify-between">
          <h3 class="text-base font-medium">
            {{ t('models.installed.title') }}
          </h3>
          <UiButton type="button" variant="outline" size="sm" :loading="checkingUpdates" @click="checkUpdatesNowAsync">
            {{ t('models.update.checkNow') }}
          </UiButton>
        </div>
        <p v-if="updateErrorKey" class="text-sm text-red-500" role="alert">
          {{ t(updateErrorKey) }}
        </p>
        <p v-if="loadErrorKey" class="text-sm text-red-500" role="alert">
          {{ t(loadErrorKey) }}
        </p>
        <p v-if="deleteErrorKey" class="text-sm text-red-500" role="alert">
          {{ t(deleteErrorKey) }}
        </p>
        <p v-if="installed.length === 0" class="text-sm text-neutral-500">
          {{ t('models.installed.empty') }}
        </p>
        <div
          v-for="model in installed"
          :key="model.id"
          class="relative flex flex-col gap-1 overflow-hidden rounded-md border border-neutral-300 p-3"
        >
          <div
            v-if="downloadStates[model.id]"
            class="pointer-events-none absolute inset-y-0 left-0 bg-blue-100/70 transition-[width] duration-150"
            :class="downloadPercent(model.id) === null ? 'animate-pulse' : ''"
            :style="{ width: downloadWidth(model.id) }"
            role="progressbar"
            :aria-valuenow="downloadPercent(model.id) ?? undefined"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-label="t('models.filePicker.downloadProgress', { done: humanBytes(downloadStates[model.id]?.bytesDownloaded ?? 0), total: humanBytes(downloadStates[model.id]?.bytesTotal ?? null) })"
          />
          <div class="relative z-10 flex flex-col gap-1">
            <div class="flex items-center justify-between gap-2">
              <span class="font-medium">{{ model.name }}</span>
              <span v-if="activeModelId === model.id" class="rounded bg-green-100 px-1.5 py-0.5 text-xs text-green-800">
                {{ t('models.installed.active') }}
              </span>
            </div>
            <div class="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-neutral-500">
              <span>{{ t(`models.installed.source.${model.sourceKind}`) }}</span>
              <span>{{ t(`models.installed.integrity.${model.integrityStatus}`) }}</span>
            </div>
            <div v-if="downloadStates[model.id]" class="text-xs text-blue-800" role="status">
              {{ t('models.filePicker.downloadProgress', { done: humanBytes(downloadStates[model.id]?.bytesDownloaded ?? 0), total: humanBytes(downloadStates[model.id]?.bytesTotal ?? null) }) }}
            </div>
            <div v-if="model.hfRevisionRef && updateStatuses[model.id]" class="text-xs">
              <span v-if="updateStatuses[model.id]?.errorCode" class="text-red-500">
                {{ t('models.update.error') }}
              </span>
              <span v-else-if="updateStatuses[model.id]?.updateAvailable" class="text-amber-600">
                {{ t('models.update.available') }} ({{ t('models.update.oldRevision') }}:
                {{ updateStatuses[model.id]?.installedRevision.slice(0, 8) }} →
                {{ t('models.update.newRevision') }}: {{ updateStatuses[model.id]?.latestRevision?.slice(0, 8) }})
              </span>
              <span v-else class="text-neutral-500">{{ t('models.update.upToDate') }}</span>
            </div>
            <div v-else-if="model.sourceKind === 'huggingface' && !model.hfRevisionRef" class="text-xs text-neutral-500">
              {{ t('models.update.notTrackable') }}
            </div>
            <div class="flex flex-wrap items-center gap-2 pt-1">
              <UiButton
                type="button"
                size="sm"
                :disabled="activeModelId === model.id"
                :loading="busyModelId === model.id"
                @click="loadModelHereAsync(model.id)"
              >
                {{ t('models.installed.load') }}
              </UiButton>
              <UiButton
                v-if="updateStatuses[model.id]?.updateAvailable"
                type="button"
                size="sm"
                variant="outline"
                :loading="installingUpdateId === model.id"
                @click="installUpdateForAsync(model.id)"
              >
                {{ t('models.update.install') }}
              </UiButton>
              <UiButton
                type="button"
                size="sm"
                variant="ghost"
                :loading="busyModelId === model.id"
                @click="deleteModelAsync(model.id)"
              >
                {{ t('models.installed.delete') }}
              </UiButton>
            </div>
          </div>
        </div>
      </div>

      <div v-else-if="activeTab === 'search'">
        <ModelsHuggingFaceFilePicker
          v-if="selectedRepo"
          :repo-id="selectedRepo.repoId"
          @installed="onFilePickerInstalled"
          @back="selectedRepo = null"
        />
        <ModelsHuggingFaceSearch v-else @select="selectedRepo = $event" />
      </div>

      <div v-else class="flex flex-col gap-2">
        <div
          v-for="entry in catalogEntries"
          :key="entry.id"
          class="relative flex items-center justify-between gap-2 overflow-hidden rounded-md border border-neutral-300 p-3"
        >
          <div
            v-if="downloadStates[entry.id]"
            class="pointer-events-none absolute inset-y-0 left-0 bg-blue-100/70 transition-[width] duration-150"
            :class="downloadPercent(entry.id) === null ? 'animate-pulse' : ''"
            :style="{ width: downloadWidth(entry.id) }"
            role="progressbar"
            :aria-valuenow="downloadPercent(entry.id) ?? undefined"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-label="t('models.filePicker.downloadProgress', { done: humanBytes(downloadStates[entry.id]?.bytesDownloaded ?? 0), total: humanBytes(downloadStates[entry.id]?.bytesTotal ?? null) })"
          />
          <div class="relative z-10 flex flex-col">
            <span class="font-medium">{{ entry.name }}</span>
            <span class="text-xs text-neutral-500">{{ entry.parameters }} · {{ entry.quantization }} · {{ t(`models.filePicker.fit.${entry.fit}`) }}</span>
            <span v-if="downloadStates[entry.id]" class="text-xs text-blue-800" role="status">
              {{ t('models.filePicker.downloadProgress', { done: humanBytes(downloadStates[entry.id]?.bytesDownloaded ?? 0), total: humanBytes(downloadStates[entry.id]?.bytesTotal ?? null) }) }}
            </span>
          </div>
          <div class="relative z-10">
            <UiButton
              type="button"
              size="sm"
              :disabled="installed.some((m) => m.id === entry.id)"
              :loading="busyModelId === entry.id"
              @click="downloadCatalogEntryAsync(entry)"
            >
              {{ installed.some((m) => m.id === entry.id) ? t('models.installed.active') : t('models.filePicker.install') }}
            </UiButton>
          </div>
        </div>
      </div>
    </template>

    <ModelsModelIntegrityDialog
      v-if="integrityDialog"
      :open="integrityDialog !== null"
      :error-kind="integrityDialog.errorKind"
      :expected-sha256="integrityDialog.expected"
      :actual-sha256="integrityDialog.actual"
      :busy="integrityBusy"
      :action-error="integrityActionError"
      @update:open="onIntegrityDialogOpenChange"
      @load-untrusted="onLoadUntrustedAsync"
      @repair-source="onRepairSourceAsync"
      @choose-other="onChooseOther"
    />
  </section>
</template>
