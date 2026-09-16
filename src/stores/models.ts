import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  useChat,
  type LoadedModelInfo,
  type ModelLoadErrorEvent,
  type ModelLoadPhase,
  type ModelLoadProgressEvent,
  type ModelLoadStatusPayload,
} from '~/composables/useChat'
import {
  parseModelIntegrityFailure,
  useModels,
  type InstalledModel,
  type ModelIntegrityFailure,
} from '~/composables/useModels'
import { useCatalog, type CatalogEntryWithFit } from '~/composables/useCatalog'
import {
  useProviders,
  type Provider,
  type ProviderModel,
} from '~/composables/useProviders'
import { useErrorString } from '~/composables/useErrorString'

export type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string }[]
}

/**
 * Model lifecycle: install/catalog/provider listing, load/unload, download
 * progress, and the pre-load integrity dialog. Exactly one model is ever
 * active at a time, so this is a plain singleton store rather than
 * something keyed by id.
 *
 * `busy` and `lastError` for the composer/send flow stay on the chat page —
 * they're shared with logic that has nothing to do with models, and Pinia
 * setup stores can't take page-local refs as constructor params. `loadModel`
 * here does not touch the page's `busy`; callers that need to block on it
 * (`newChat`) check `modelLoadPending` directly instead.
 */
export const useModelsStore = defineStore('models', () => {
  const chat = useChat()
  const models = useModels()
  const catalog = useCatalog()
  const providers = useProviders()
  const { t } = useI18n()
  const { errString } = useErrorString()

  const activeModel = ref<LoadedModelInfo | null>(null)
  const installedModels = ref<InstalledModel[]>([])
  const catalogEntries = ref<CatalogEntryWithFit[]>([])
  const providerList = ref<Provider[]>([])
  const providerModels = ref<Record<string, ProviderModel[]>>({})
  const modelLoadPending = ref(false)
  const lastError = ref<string | null>(null)

  const downloadingId = ref<string | null>(null)
  const downloadProgressBytes = ref(0)
  const downloadTotalBytes = ref<number | null>(null)

  // Structured loading state driven by the `model-load-progress` event. The
  // composer stays available for drafting while sending remains blocked
  // until the load reaches `ready`.
  const loadingPhase = ref<ModelLoadPhase | null>(null)
  const loadingModelName = ref('')
  const loadingProviderName = ref<string | null>(null)
  const loadErrorModelId = ref<string | null>(null)

  const integrityDialog = ref<ModelIntegrityFailure | null>(null)
  const integrityBusy = ref(false)
  const integrityActionError = ref<string | null>(null)

  const noModelsInstalled = computed(
    () =>
      installedModels.value.length === 0 &&
      Object.values(providerModels.value).every((list) => list.length === 0),
  )

  // Read through a computed rather than `activeModel?.modelId` directly in
  // the template — vue-tsc narrows `activeModel` to `never` at the model
  // picker's `v-else-if="!activeModel"` (a chained-`v-if` control-flow
  // quirk), which a plain computed's independent return type sidesteps.
  const activeModelId = computed(() => activeModel.value?.modelId ?? '')

  /** Groups selectable models by provider for the picker's <optgroup>. */
  const modelGroups = computed<ModelGroup[]>(() => {
    const localGroup: ModelGroup | null =
      installedModels.value.length > 0
        ? {
            providerId: 'local',
            providerName: t('chat.model.local'),
            models: installedModels.value.map((m) => ({
              id: m.id,
              name: m.name,
            })),
          }
        : null

    const remoteGroups = providerList.value
      .filter((p) => p.kind === 'api_key')
      .map<ModelGroup>((p) => ({
        providerId: p.id,
        providerName: p.name,
        models: (providerModels.value[p.id] ?? []).map((m) => ({
          id: m.id,
          name: m.name,
        })),
      }))
      .filter((g) => g.models.length > 0)

    return localGroup ? [localGroup, ...remoteGroups] : remoteGroups
  })

  /** Localised label for the current loading phase, if any. */
  const loadingLabel = computed<string | null>(() => {
    const phase = loadingPhase.value
    if (!phase || phase === 'ready') return null
    const key =
      phase === 'connecting'
        ? 'chat.loading.connecting'
        : phase === 'cuda-jit-warmup'
          ? 'chat.loading.cudaJitWarmup'
          : 'chat.loading.loading'
    return t(key, {
      modelName: loadingModelName.value,
      providerName: loadingProviderName.value ?? '',
    })
  })

  /** Refreshes the installed models and their catalog metadata together. */
  async function refreshInstalledAndCatalog() {
    installedModels.value = await models.listInstalledAsync()
    catalogEntries.value = await catalog.listAsync()
  }

  /** Refreshes the provider list and re-fetches api_key model caches. */
  async function refreshProviders() {
    providerList.value = await providers.listAsync()
    const next: Record<string, ProviderModel[]> = {}
    await Promise.all(
      providerList.value
        .filter((p) => p.kind === 'api_key')
        .map(async (p) => {
          next[p.id] = await providers.listModelsAsync(p.id)
        }),
    )
    providerModels.value = next
  }

  /** Refreshes the backend's currently active model snapshot. */
  async function refreshActiveModel() {
    activeModel.value = await chat.activeModelInfoAsync()
  }

  /**
   * Detects the three structured integrity error kinds `load_model` can
   * return (spec 005 §"load_model und lokale Integritätsprüfung") and opens
   * the decision dialog instead of showing a plain error string. Returns
   * `false` for every other error so the caller falls back to `lastError`.
   */
  function openIntegrityDialog(modelId: string, e: unknown): boolean {
    const failure = parseModelIntegrityFailure(modelId, e)
    if (!failure) return false
    integrityDialog.value = failure
    return true
  }

  /** Loads the selected model (local or api_key composite id). */
  async function loadModel(id: string) {
    lastError.value = null
    loadErrorModelId.value = null
    modelLoadPending.value = true
    try {
      activeModel.value = await chat.loadModelAsync(id)
    } catch (e: unknown) {
      if (!openIntegrityDialog(id, e)) {
        lastError.value = errString(e)
      }
      loadingPhase.value = null
      activeModel.value = null
    } finally {
      modelLoadPending.value = false
    }
  }

  /** "Trotzdem als unsicher laden" — bypasses the hash check for this load only. */
  async function onIntegrityLoadUntrusted() {
    if (!integrityDialog.value) return
    modelLoadPending.value = true
    integrityBusy.value = true
    integrityActionError.value = null
    try {
      activeModel.value = await chat.loadModelWithIntegrityOverrideAsync(
        integrityDialog.value.modelId,
      )
      integrityDialog.value = null
      await refreshInstalledAndCatalog()
    } catch (e) {
      integrityActionError.value = errString(e)
    } finally {
      integrityBusy.value = false
      modelLoadPending.value = false
    }
  }

  /** "Erneut herunterladen / neu importieren" — re-installs from the model's stored HF source. */
  async function onIntegrityRepairSource() {
    const dialog = integrityDialog.value
    if (!dialog) return
    const model = installedModels.value.find((m) => m.id === dialog.modelId)
    integrityBusy.value = true
    integrityActionError.value = null
    try {
      if (
        model?.sourceKind === 'huggingface' &&
        model.hfRepo &&
        model.hfFilename
      ) {
        await models.downloadFromHfAsync({
          repoId: model.hfRepo,
          filename: model.hfFilename,
          // Repair reinstalls the stored source: the tracked ref when there
          // is one, otherwise the pinned commit — never an implicit `main`.
          revision: model.hfRevisionRef ?? model.hfRevision ?? undefined,
          name: model.name,
          contextWindow: model.contextWindow,
          // Without this the backend's same-source short-circuit returns the
          // existing row and the corrupt file is never replaced.
          forceRepair: true,
        })
        integrityDialog.value = null
        await refreshInstalledAndCatalog()
      } else {
        integrityActionError.value = t('models.integrityDialog.actionFailed')
      }
    } catch (e) {
      integrityActionError.value = errString(e)
    } finally {
      integrityBusy.value = false
    }
  }

  /** "Anderes Modell auswählen" — just closes the dialog; the picker is already visible. */
  function onIntegrityChooseOther() {
    integrityDialog.value = null
  }

  /** Clears integrity-dialog state when the dialog is dismissed. */
  function onIntegrityDialogOpenChange(open: boolean) {
    if (!open) {
      integrityDialog.value = null
      integrityActionError.value = null
    }
  }

  /** Downloads a catalog model, refreshes the lists, and loads the result. */
  async function downloadCatalogEntry(entry: CatalogEntryWithFit) {
    lastError.value = null
    downloadingId.value = entry.id
    downloadProgressBytes.value = 0
    downloadTotalBytes.value = entry.approx_size_bytes
    try {
      await models.downloadFromCatalogAsync(entry.id)
      await refreshInstalledAndCatalog()
      await loadModel(entry.id)
    } catch (e: unknown) {
      lastError.value = errString(e)
    } finally {
      downloadingId.value = null
    }
  }

  /** Applies an incremental model-load progress event to store state. */
  function applyLoadProgress(e: ModelLoadProgressEvent) {
    loadErrorModelId.value = null
    loadingPhase.value = e.phase
    loadingModelName.value = e.modelName
    loadingProviderName.value = e.providerName ?? null
    if (e.phase === 'ready') {
      loadingPhase.value = null
      void refreshActiveModel().catch((e: unknown) => {
        lastError.value = errString(e)
      })
    }
  }

  /** Reconciles store state with a complete model-load status snapshot. */
  function applyLoadStatus(status: ModelLoadStatusPayload) {
    if (status.status === 'loading') {
      loadErrorModelId.value = null
      loadingPhase.value = status.phase
      loadingModelName.value = status.modelName
      loadingProviderName.value = status.providerName ?? null
    } else {
      loadingPhase.value = null
      loadingModelName.value =
        'modelName' in status ? (status.modelName ?? '') : ''
      loadingProviderName.value = null
      loadErrorModelId.value =
        status.status === 'error' ? (status.modelId ?? null) : null
      if (status.status === 'error') lastError.value = t('chat.loading.error')
      if (status.status === 'ready') {
        void refreshActiveModel().catch((e: unknown) => {
          lastError.value = errString(e)
        })
      }
    }
  }

  /** Records a terminal model-load error event. */
  function applyLoadError(event: ModelLoadErrorEvent) {
    loadingPhase.value = null
    loadErrorModelId.value = event.modelId ?? null
    lastError.value = t('chat.loading.error')
  }

  /** Retries the model most recently associated with a load error. */
  async function retryModelLoad() {
    const modelId = loadErrorModelId.value
    if (!modelId) return
    await loadModel(modelId)
  }

  /** Fetches the model-load snapshot and every installed/catalog/provider list. */
  async function initialize() {
    const status = await chat.modelLoadStatusAsync()
    if (status) applyLoadStatus(status)
    await refreshActiveModel()
    await refreshInstalledAndCatalog()
    await refreshProviders()
  }

  let stopped = false
  const unlisteners: UnlistenFn[] = []

  /**
   * Subscribes to the model-load + download-progress events. Mirrors the
   * "registration can finish after the page already stopped listening"
   * handling the chat page uses for its own events: `stopListening` may run
   * before one of these promises resolves, in which case the late unlisten
   * is disposed immediately instead of leaking.
   */
  async function startListening() {
    stopped = false
    await Promise.all(
      [
        chat.onModelLoadProgress(applyLoadProgress),
        chat.onModelLoadStatus(applyLoadStatus),
        chat.onModelLoadError(applyLoadError),
        models.onDownloadProgress((e) => {
          if (downloadingId.value === e.modelId) {
            downloadProgressBytes.value = e.bytesDownloaded
            downloadTotalBytes.value = e.bytesTotal
          }
        }),
      ].map(async (subscription) => {
        const unlisten = await subscription
        if (stopped) unlisten()
        else unlisteners.push(unlisten)
      }),
    )
  }

  /** Stops future subscriptions and disposes every active listener. */
  function stopListening() {
    stopped = true
    for (const unlisten of unlisteners.splice(0)) unlisten()
  }

  return {
    activeModel,
    installedModels,
    catalogEntries,
    providerList,
    providerModels,
    modelLoadPending,
    lastError,
    downloadingId,
    downloadProgressBytes,
    downloadTotalBytes,
    loadingPhase,
    loadingModelName,
    loadingProviderName,
    loadErrorModelId,
    integrityDialog,
    integrityBusy,
    integrityActionError,
    noModelsInstalled,
    activeModelId,
    modelGroups,
    loadingLabel,
    refreshInstalledAndCatalog,
    refreshProviders,
    refreshActiveModel,
    loadModel,
    onIntegrityLoadUntrusted,
    onIntegrityRepairSource,
    onIntegrityChooseOther,
    onIntegrityDialogOpenChange,
    downloadCatalogEntry,
    retryModelLoad,
    initialize,
    startListening,
    stopListening,
  }
})
