import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type {
  LoadedModelInfo,
  ModelLoadErrorEvent,
  ModelLoadPhase,
  ModelLoadProgressEvent,
  ModelLoadStatusPayload,
} from '~/composables/useChat'
import type { CatalogEntryWithFit } from '~/composables/useCatalog'
import type { ResolveDefaultModelResult } from '~/composables/usePreferences'
import { useModelInventory } from '~/composables/useModelInventory'
import { useModelIntegrity } from '~/composables/useModelIntegrity'
import { useReasoningPreference } from '~/composables/useReasoningPreference'

/**
 * Model lifecycle: install/catalog/provider listing, load/unload, download
 * progress, and the pre-load integrity dialog. Exactly one model is ever
 * active at a time, so this is a plain singleton store rather than
 * something keyed by id. The lists/picker grouping and the integrity dialog
 * live in `useModelInventory`/`useModelIntegrity` (spaex 500-LoC boundary);
 * this store owns the load lifecycle and re-exports their state unchanged.
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
  const {
    resolveDefaultModelAsync,
    getPrefAsync,
    setPrefAsync,
    clearPrefAsync,
  } = usePreferences()
  const { currentDeviceInfoAsync } = useDevice()
  const { t } = useI18n()
  const { errString } = useErrorString()

  const activeModel = ref<LoadedModelInfo | null>(null)
  const modelLoadPending = ref(false)
  const lastError = ref<string | null>(null)
  const setError = (message: string) => {
    lastError.value = message
  }
  // This vault device's uuid — the scope of the per-model reasoning
  // preference. Resolved once at the start of `initialize()`; nothing
  // device-scoped is read or written before it is known.
  const vaultDeviceUuid = ref<string | null>(null)

  const {
    installedModels,
    catalogEntries,
    providerList,
    providerModels,
    noModelsInstalled,
    modelGroups,
    findModelName,
    capabilitiesFor,
    refreshInstalledAndCatalog,
    refreshProviders,
  } = useModelInventory({ models, catalog, providers, t, errString, setError })

  const downloadingId = ref<string | null>(null)
  const downloadProgressBytes = ref(0)
  const downloadTotalBytes = ref<number | null>(null)

  // Structured loading state driven by the `model-load-progress` event. The
  // composer stays available for drafting while sending remains blocked
  // until the load reaches `ready`.
  const loadingPhase = ref<ModelLoadPhase | null>(null)
  const loadingModelName = ref('')
  const loadingProviderName = ref<string | null>(null)
  // Which model `loadModel()` is currently targeting — set synchronously,
  // before the `load_model` round trip even starts, purely from
  // `modelGroups` (already fetched). Backs `displayModelId`/
  // `displayModelName` below so the composer reflects a fresh pick
  // immediately instead of only once the load resolves or the backend's
  // first `model-load-progress` event arrives.
  const loadingModelId = ref<string | null>(null)
  const loadErrorModelId = ref<string | null>(null)
  let loadingModelToken = 0

  // Read through a computed rather than `activeModel?.modelId` directly in
  // the template — vue-tsc narrows `activeModel` to `never` at the model
  // picker's `v-else-if="!activeModel"` (a chained-`v-if` control-flow
  // quirk), which a plain computed's independent return type sidesteps.
  const activeModelId = computed(() => activeModel.value?.modelId ?? '')

  // What the composer's model control should show: the model a load is
  // currently targeting, if any, else the actually-active one. Without
  // this, picking a new model left the composer showing the previous one
  // (or nothing) until `load_model`'s round trip resolved — `activeModel`
  // itself stays untouched until then, on purpose (`sendDisabled` etc.
  // must keep treating a pending load as "not ready"). `loadingModelId` is
  // set/cleared in lockstep by every load path — `loadModel`,
  // `onIntegrityLoadUntrusted`, and the backend-driven
  // `applyLoadProgress`/`applyLoadStatus`/`applyLoadError` handlers below
  // (the vault-open preload never calls `loadModel` at all) — so checking
  // it alone is sufficient; the extra `modelLoadPending.value &&` this used
  // to require was redundant for the frontend-initiated path and actively
  // wrong for the backend-initiated one.
  const displayModelId = computed(() =>
    loadingModelId.value ? loadingModelId.value : activeModelId.value,
  )
  const displayModelName = computed(() =>
    loadingModelId.value
      ? loadingModelName.value
      : (activeModel.value?.name ?? undefined),
  )

  // The displayed model's cached capabilities, read from the lists above —
  // no IPC and no second cache. `undefined` = no resolved row.
  const displayModelRecord = computed(() =>
    capabilitiesFor(displayModelId.value),
  )
  const displayModelCapabilities = computed(
    () => displayModelRecord.value ?? null,
  )
  const { effortLevel, effortOptions, effortState, updateEffortLevel } =
    useReasoningPreference({
      modelId: displayModelId,
      capabilities: displayModelRecord,
      deviceUuid: vaultDeviceUuid,
      preferences: { getPrefAsync, setPrefAsync, clearPrefAsync },
      errString,
      setError,
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

  /** Refreshes the backend's currently active model snapshot. */
  async function refreshActiveModel() {
    activeModel.value = await chat.activeModelInfoAsync()
  }

  function beginLoadingModel(id: string, name: string): number {
    loadingModelToken += 1
    loadingModelId.value = id
    loadingModelName.value = name
    return loadingModelToken
  }

  function clearLoadingModel(token?: number) {
    if (token !== undefined && token !== loadingModelToken) return
    loadingModelToken += 1
    loadingModelId.value = null
    loadingModelName.value = ''
  }

  async function finishLoadingModel(token: number) {
    try {
      await refreshActiveModel()
      clearLoadingModel(token)
    } catch (e: unknown) {
      lastError.value = errString(e)
    }
  }

  const {
    integrityDialog,
    integrityBusy,
    integrityActionError,
    openIntegrityDialog,
    onIntegrityLoadUntrusted,
    onIntegrityRepairSource,
    onIntegrityChooseOther,
    onIntegrityDialogOpenChange,
  } = useModelIntegrity({
    chat,
    models,
    t,
    errString,
    installedModels,
    activeModel,
    modelLoadPending,
    findModelName,
    beginLoadingModel,
    clearLoadingModel,
    refreshInstalledAndCatalog,
    setError,
  })

  /** Loads the selected model (local or api_key composite id). */
  async function loadModel(id: string) {
    lastError.value = null
    loadErrorModelId.value = null
    modelLoadPending.value = true
    const loadToken = beginLoadingModel(id, findModelName(id))
    try {
      activeModel.value = await chat.loadModelAsync(id)
    } catch (e: unknown) {
      if (!openIntegrityDialog(id, e)) {
        lastError.value = errString(e)
      }
      loadingPhase.value = null
      // The backend never touches its existing session unless this load
      // fully succeeds (`load_model_inner` only swaps `chat.session` at
      // the very end) — so unconditionally nulling `activeModel` here is
      // wrong whenever this call was switching away from an already-good
      // model, including when the failure is the backend rejecting this
      // request in favor of a *different* load that's genuinely in
      // flight (the operation mutex, or the vault-open preload's own
      // generation counter racing this call — see
      // `autoLoadFirstAvailableModel`). Re-read backend truth instead of
      // guessing "nothing is active".
      await refreshActiveModel().catch(() => {})
    } finally {
      modelLoadPending.value = false
      clearLoadingModel(loadToken)
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
    // Backend-driven loads — chiefly the vault-open background preload,
    // which never goes through `loadModel()` at all — would otherwise
    // leave `loadingModelId` unset, so `displayModelId`/`displayModelName`
    // fall back to the (still empty) active model and the composer shows
    // "Choose a model" the whole time a preload that the loading banner
    // already reports is running.
    const loadToken = beginLoadingModel(e.modelId, e.modelName)
    loadingProviderName.value = e.providerName ?? null
    if (e.phase === 'ready') {
      loadingPhase.value = null
      void finishLoadingModel(loadToken)
    }
  }

  /** Reconciles store state with a complete model-load status snapshot. */
  function applyLoadStatus(status: ModelLoadStatusPayload) {
    if (status.status === 'loading') {
      loadErrorModelId.value = null
      loadingPhase.value = status.phase
      loadingModelName.value = status.modelName
      beginLoadingModel(status.modelId, status.modelName)
      loadingProviderName.value = status.providerName ?? null
    } else {
      loadingPhase.value = null
      loadingModelName.value =
        'modelName' in status ? (status.modelName ?? '') : ''
      loadingProviderName.value = null
      loadErrorModelId.value =
        status.status === 'error' ? (status.modelId ?? null) : null
      if (status.status === 'error') {
        clearLoadingModel()
        lastError.value = t('chat.loading.error')
      }
      if (status.status === 'ready') {
        const loadToken = beginLoadingModel(status.modelId, status.modelName)
        void finishLoadingModel(loadToken)
      }
      if (status.status === 'idle') clearLoadingModel()
    }
  }

  /** Records a terminal model-load error event. */
  function applyLoadError(event: ModelLoadErrorEvent) {
    loadingPhase.value = null
    clearLoadingModel()
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
    lastError.value = null
    loadErrorModelId.value = null
    // First, so the displayed model's saved reasoning option can load as soon
    // as the active model is known below.
    try {
      vaultDeviceUuid.value = (await currentDeviceInfoAsync()).vaultDeviceUuid
    } catch (e: unknown) {
      lastError.value = errString(e)
    }
    if (!integrityBusy.value) {
      integrityDialog.value = null
      integrityActionError.value = null
    }
    const status = await chat.modelLoadStatusAsync()
    if (status) applyLoadStatus(status)
    await refreshActiveModel()
    await refreshInstalledAndCatalog()
    await refreshProviders()
    await autoLoadFirstAvailableModel()
  }

  /**
   * Session-start fallback (spec 002 §FR-014's last step): once every list
   * above is fresh, auto-loads whichever model the backend resolver picks
   * — this device's last-active model, then its default, then the vault
   * default, then simply the first available one. A newly-connected
   * provider's models only exist in that resolver's view *after*
   * `refreshProviders()` has fetched and cached them once, which is why
   * this runs here rather than as part of the vault-open background
   * preload (`start_default_model_preload`, Rust) — that one only sees
   * whatever was already cached before this page ever mounted. A `null`
   * result (`source: 'none'`) means nothing is loadable yet, so this is a
   * no-op; the composer's own model control remains the only place to
   * pick one by hand.
   */
  function shouldSkipAutoLoad(): boolean {
    return (
      activeModel.value !== null ||
      modelLoadPending.value ||
      loadingPhase.value !== null
    )
  }

  async function autoLoadFirstAvailableModel() {
    if (shouldSkipAutoLoad()) return
    let result: ResolveDefaultModelResult
    try {
      result = await resolveDefaultModelAsync()
    } catch (e) {
      // A genuine resolver failure (backend/IPC error) must not look
      // identical to "nothing is loadable yet" — the latter is a normal,
      // silent no-op; the former is worth surfacing so it doesn't get
      // mistaken for an empty vault during triage.
      lastError.value = errString(e)
      return
    }
    // Re-check: the resolver call above is a real IPC/DB round trip, and
    // something else — a manual pick from the composer, the vault-open
    // background preload reaching this device first — may have started
    // or finished a load while we were awaiting it.
    if (!result.modelId || shouldSkipAutoLoad()) return
    await loadModel(result.modelId)
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
    displayModelId,
    displayModelName,
    displayModelCapabilities,
    effortLevel,
    effortOptions,
    effortState,
    updateEffortLevel,
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
