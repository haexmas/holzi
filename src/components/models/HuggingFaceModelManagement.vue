<script setup lang="ts">
/*
 * Maintainability exception (spaex 500-LoC rule): ~270 lines of script
 * plus ~290 of template covering three tabs that share `installed`,
 * `activeModelId` and the download-progress subscription. `installed`,
 * `catalogEntries`, `activeModelId` and the integrity-dialog flow are
 * `useModelsStore()` state now, not owned here (they used to be a second,
 * independent copy of exactly what the chat page's store already
 * tracks) — but splitting the tabs still means lifting the
 * component-local half of that shared state (`busyModelId`,
 * `downloadStates`, the HF-search/update-check state) into props and
 * events without any test to hold the wiring in place, since this
 * component has no executable coverage yet.
 *
 * Concrete split plan: add a replay test for the installed-model tab
 * first, then extract the catalog-download tab (`catalogEntries`,
 * `downloadCatalogEntryAsync`, `downloadPercent`, `downloadWidth`,
 * `humanBytes` and their markup) and the update-check panel
 * (`updateStatuses`, `checkUpdatesNowAsync`, `installUpdateForAsync`)
 * into child components, leaving the installed list, its delete/load
 * actions and the integrity dialog here.
 */
import type { UnlistenFn } from '@tauri-apps/api/event'
import type {
  HuggingFaceModelResult,
  HuggingFaceUpdateStatus,
} from '~/composables/useHuggingFace'
import type {
  DownloadProgressEvent,
  InstalledModel,
} from '~/composables/useModels'
import type { CatalogEntryWithFit } from '~/composables/useCatalog'

const { t } = useI18n()
const { onDownloadProgress, onDownloadComplete } = useModels()
const checkUpdates = useActionOrThrow('settings.models.checkUpdates')
const installUpdate = useActionOrThrow('settings.models.installUpdate')
const downloadCatalog = useActionOrThrow('settings.models.downloadCatalog')
const deleteModel = useActionOrThrow('settings.models.delete')
const selectModel = useAction('chat.model.select')
const decideIntegrity = useAction('chat.modelIntegrity.decide')

// Installed/catalog lists, the active model and the integrity-dialog flow
// all come straight from the same store the chat page uses — this used
// to be a second, independent copy of that exact state (its own
// `activeModelId`, its own `loadModelAsync`/integrity-dialog handling),
// which meant a fix to one side (e.g. the active model's display name)
// silently didn't apply to the other.
const modelStore = useModelsStore()
const {
  installedModels: installed,
  catalogEntries,
  activeModelId,
  integrityDialog,
  integrityBusy,
  integrityActionError,
} = storeToRefs(modelStore)
const {
  refreshInstalledAndCatalog,
  refreshActiveModel,
  onIntegrityDialogOpenChange,
} = modelStore

type Tab = 'catalog' | 'search' | 'installed'
const activeTab = ref<Tab>('installed')

const listErrorKey = ref<string | null>(null)
const listErrorDetail = ref<string | null>(null)
const loading = ref(true)

const selectedRepo = ref<HuggingFaceModelResult | null>(null)

const updateStatuses = ref<Record<string, HuggingFaceUpdateStatus>>({})
const checkingUpdates = ref(false)
const updateErrorKey = ref<string | null>(null)
const updateErrorDetail = ref<string | null>(null)
const installingUpdateId = ref<string | null>(null)

const busyModelId = ref<string | null>(null)
const deleteErrorKey = ref<string | null>(null)
// The store's own `lastError` is already a resolved, display-ready
// string (unlike `listErrorKey`/`deleteErrorKey` below, which are i18n
// keys rendered through `t()`) — `chat.model.select` runs the store's `loadModel`,
// so its failure reads the same way the chat page shows it.
const loadErrorMessage = ref<string | null>(null)
const downloadStates = ref<Record<string, DownloadProgressEvent>>({})

let unlistenDownloadProgress: UnlistenFn | null = null
let unlistenDownloadComplete: UnlistenFn | null = null

/**
 * Switches tabs and drops the picked repository with it.
 *
 * A named handler rather than a multi-statement template expression: Vue
 * collapses a template attribute's newlines before parsing it, so the
 * inline form needs `;` separators that Prettier's `semi: false` strips,
 * leaving markup the SFC compiler rejects.
 */
function selectTab(tab: Tab) {
  activeTab.value = tab
  selectedRepo.value = null
}

async function reloadAsync() {
  loading.value = true
  listErrorKey.value = null
  listErrorDetail.value = null
  try {
    await Promise.all([refreshInstalledAndCatalog(), refreshActiveModel()])
  } catch (e) {
    listErrorKey.value = hfErrorKey(e)
    listErrorDetail.value = hfErrorDetail(e)
  } finally {
    loading.value = false
  }
}

async function checkUpdatesNowAsync() {
  checkingUpdates.value = true
  updateErrorKey.value = null
  updateErrorDetail.value = null
  try {
    const { statuses } = (await checkUpdates()) as {
      statuses: HuggingFaceUpdateStatus[]
    }
    updateStatuses.value = Object.fromEntries(
      statuses.map((s) => [s.modelId, s]),
    )
  } catch (e) {
    updateErrorKey.value = hfErrorKey(e)
    updateErrorDetail.value = hfErrorDetail(e)
  } finally {
    checkingUpdates.value = false
  }
}

async function installUpdateForAsync(modelId: string) {
  installingUpdateId.value = modelId
  updateErrorKey.value = null
  updateErrorDetail.value = null
  try {
    await installUpdate({ modelId })
    await reloadAsync()
    await checkUpdatesNowAsync()
  } catch (e) {
    updateErrorKey.value = hfErrorKey(e)
    updateErrorDetail.value = hfErrorDetail(e)
  } finally {
    installingUpdateId.value = null
    clearDownloadState(modelId)
  }
}

async function downloadCatalogEntryAsync(entry: CatalogEntryWithFit) {
  busyModelId.value = entry.id
  listErrorKey.value = null
  listErrorDetail.value = null
  try {
    await downloadCatalog({ entryId: entry.id })
    await reloadAsync()
  } catch (e) {
    listErrorKey.value = hfErrorKey(e)
    listErrorDetail.value = hfErrorDetail(e)
  } finally {
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
  const { [modelId]: _removed, ...next } = downloadStates.value
  downloadStates.value = next
}

function downloadPercent(modelId: string): number | null {
  const state = downloadStates.value[modelId]
  if (!state || state.bytesTotal === null || state.bytesTotal <= 0) return null
  return Math.min(
    100,
    Math.max(0, Math.round((state.bytesDownloaded / state.bytesTotal) * 100)),
  )
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
  if (!confirm(t('models.installed.deleteConfirm'))) return
  busyModelId.value = id
  deleteErrorKey.value = null
  try {
    await deleteModel({ modelId: id })
    await reloadAsync()
  } catch (e) {
    deleteErrorKey.value = hfErrorKey(e)
  } finally {
    busyModelId.value = null
  }
}

/**
 * The load itself, and any integrity failure it hits, are handled by the
 * store's own `loadModel` — it never throws, only ever sets its
 * `integrityDialog` (handled by the dialog below, wired to the store's
 * own actions) or `lastError`. `activeModelId` picks up the new model on
 * its own once `loadModel` resolves, being a computed over the store's
 * `activeModel`.
 */
async function loadModelHereAsync(id: string) {
  busyModelId.value = id
  loadErrorMessage.value = null
  try {
    await selectModel({ modelId: id })
    if (!integrityDialog.value && modelStore.lastError) {
      loadErrorMessage.value = modelStore.lastError
    }
  } finally {
    busyModelId.value = null
  }
}

/**
 * "Erneut herunterladen" — delegates the actual repair to the store, then
 * clears this component's own `downloadStates` entry for the repaired
 * model. The store has no way to do this itself: `downloadStates` (and the
 * progress-bar overlay it drives) is private to this component, populated
 * by its own `onDownloadProgress`/`onDownloadComplete` listeners — a
 * repair re-downloads under the same model id, so without this the
 * card's progress overlay is left showing stale "download in progress" /
 * 100% state indefinitely (until the component happens to remount).
 */
async function onRepairSourceAsync() {
  const modelId = integrityDialog.value?.modelId
  await decideIntegrity({ decision: 'repairSource' })
  if (modelId) clearDownloadState(modelId)
}

/** Closing the dialog after "pick another model" also returns to the installed tab. */
function onChooseOther() {
  void decideIntegrity({ decision: 'chooseOther' })
  activeTab.value = 'installed'
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
        v-for="tab in ['installed', 'search', 'catalog'] as Tab[]"
        :key="tab"
        type="button"
        role="tab"
        :aria-selected="activeTab === tab"
        class="px-3 py-2 text-sm"
        :class="
          activeTab === tab
            ? 'border-b-2 border-blue-500 font-medium'
            : 'text-neutral-500'
        "
        @click="selectTab(tab)"
      >
        {{ t(`models.management.tabs.${tab}`) }}
      </button>
    </div>

    <p v-if="listErrorKey" class="text-sm text-red-500" role="alert">
      {{ t(listErrorKey) }}
      <span v-if="listErrorDetail" class="block text-xs">{{
        listErrorDetail
      }}</span>
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
          <UiButton
            type="button"
            variant="outline"
            size="sm"
            :loading="checkingUpdates"
            @click="checkUpdatesNowAsync"
          >
            {{ t('models.update.checkNow') }}
          </UiButton>
        </div>
        <p v-if="updateErrorKey" class="text-sm text-red-500" role="alert">
          {{ t(updateErrorKey) }}
          <span v-if="updateErrorDetail" class="block text-xs">{{
            updateErrorDetail
          }}</span>
        </p>
        <p v-if="loadErrorMessage" class="text-sm text-red-500" role="alert">
          {{ loadErrorMessage }}
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
            :aria-label="
              t('models.filePicker.downloadProgress', {
                done: humanBytes(
                  downloadStates[model.id]?.bytesDownloaded ?? 0,
                ),
                total: humanBytes(downloadStates[model.id]?.bytesTotal ?? null),
              })
            "
          />
          <div class="relative z-10 flex flex-col gap-1">
            <div class="flex items-center justify-between gap-2">
              <span class="font-medium">{{ model.name }}</span>
              <span
                v-if="activeModelId === model.id"
                class="rounded bg-green-100 px-1.5 py-0.5 text-xs text-green-800"
              >
                {{ t('models.installed.active') }}
              </span>
            </div>
            <div
              class="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-neutral-500"
            >
              <span>{{
                t(`models.installed.source.${model.sourceKind}`)
              }}</span>
              <span>{{
                t(`models.installed.integrity.${model.integrityStatus}`)
              }}</span>
            </div>
            <div
              v-if="downloadStates[model.id]"
              class="text-xs text-blue-800"
              role="status"
            >
              {{
                t('models.filePicker.downloadProgress', {
                  done: humanBytes(
                    downloadStates[model.id]?.bytesDownloaded ?? 0,
                  ),
                  total: humanBytes(
                    downloadStates[model.id]?.bytesTotal ?? null,
                  ),
                })
              }}
            </div>
            <div
              v-if="model.hfRevisionRef && updateStatuses[model.id]"
              class="text-xs"
            >
              <span
                v-if="updateStatuses[model.id]?.errorCode"
                class="text-red-500"
              >
                {{ t('models.update.error') }}
              </span>
              <span
                v-else-if="updateStatuses[model.id]?.updateAvailable"
                class="text-amber-600"
              >
                {{ t('models.update.available') }} ({{
                  t('models.update.oldRevision')
                }}:
                {{ updateStatuses[model.id]?.installedRevision.slice(0, 8) }} →
                {{ t('models.update.newRevision') }}:
                {{ updateStatuses[model.id]?.latestRevision?.slice(0, 8) }})
              </span>
              <span v-else class="text-neutral-500">{{
                t('models.update.upToDate')
              }}</span>
            </div>
            <div
              v-else-if="
                model.sourceKind === 'huggingface' && !model.hfRevisionRef
              "
              class="text-xs text-neutral-500"
            >
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
          :allowed-filenames="selectedRepo.files.map((file) => file.filename)"
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
            :aria-label="
              t('models.filePicker.downloadProgress', {
                done: humanBytes(
                  downloadStates[entry.id]?.bytesDownloaded ?? 0,
                ),
                total: humanBytes(downloadStates[entry.id]?.bytesTotal ?? null),
              })
            "
          />
          <div class="relative z-10 flex flex-col">
            <span class="font-medium">{{ entry.name }}</span>
            <span class="text-xs text-neutral-500"
              >{{ entry.parameters }} · {{ entry.quantization }} ·
              {{ t(`models.filePicker.fit.${entry.fit}`) }}</span
            >
            <span
              v-if="downloadStates[entry.id]"
              class="text-xs text-blue-800"
              role="status"
            >
              {{
                t('models.filePicker.downloadProgress', {
                  done: humanBytes(
                    downloadStates[entry.id]?.bytesDownloaded ?? 0,
                  ),
                  total: humanBytes(
                    downloadStates[entry.id]?.bytesTotal ?? null,
                  ),
                })
              }}
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
              {{
                installed.some((m) => m.id === entry.id)
                  ? t('models.installed.active')
                  : t('models.filePicker.install')
              }}
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
      @load-untrusted="decideIntegrity({ decision: 'loadUntrusted' })"
      @repair-source="onRepairSourceAsync"
      @choose-other="onChooseOther"
    />
  </section>
</template>
