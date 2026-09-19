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
import type {
  InstalledModel,
  ModelIntegrityFailure,
} from '~/composables/useModels'
import type { CatalogEntryWithFit } from '~/composables/useCatalog'
import type { Provider, ProviderModel } from '~/composables/useProviders'
import type { ResolveDefaultModelResult } from '~/composables/usePreferences'

export type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string; disabled?: boolean }[]
}

/**
 * The two `cli_delegate` vendors (spec 007-cli-delegate). Always shown in
 * the picker, connected or not — unlike `api_key` providers, which only
 * appear once a row (and models) exist, a delegate vendor is a fixed,
 * known option the user can discover before ever connecting one
 * (spec.md FR-004, Acceptance Scenario 2).
 */
const DELEGATE_VENDORS = ['claude', 'codex'] as const

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
  const { resolveDefaultModelAsync } = usePreferences()
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
  // Which model `loadModel()` is currently targeting — set synchronously,
  // before the `load_model` round trip even starts, purely from
  // `modelGroups` (already fetched). Backs `displayModelId`/
  // `displayModelName` below so the composer reflects a fresh pick
  // immediately instead of only once the load resolves or the backend's
  // first `model-load-progress` event arrives.
  const loadingModelId = ref<string | null>(null)
  const loadErrorModelId = ref<string | null>(null)
  let loadingModelToken = 0

  const integrityDialog = ref<ModelIntegrityFailure | null>(null)
  const integrityBusy = ref(false)
  const integrityActionError = ref<string | null>(null)

  // A connected `cli_delegate` provider's one cached model
  // (`refreshProviders` below) already lands in `providerModels`, the
  // same way an `api_key` provider's models do — so this check needs no
  // separate delegate-specific case.
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

    // A connected `cli_delegate` provider gets real cached model rows
    // (`<providerId>:<remoteId>`) via the same `list_models`/
    // `replace_provider_models` refresh path `api_key` providers already
    // use (`providers/mod.rs::compose_model_row`,
    // `adapters/cli_delegate/mod.rs::list_models`) — so it flows through
    // `remoteGroups` unchanged, no separate synthesis needed for the
    // connected case. Claude's rows come straight from Anthropic's own
    // `/v1/models` API (e.g. `<uuid>:claude-opus-5`), Codex still gets one
    // synthetic `<uuid>:codex` row.
    const remoteGroups = providerList.value
      .filter((p) => p.kind === 'api_key' || p.kind === 'cli_delegate')
      .map<ModelGroup>((p) => ({
        providerId: p.id,
        providerName: p.name,
        models: (providerModels.value[p.id] ?? []).map((m) => ({
          id: m.id,
          name: m.name,
        })),
      }))
      .filter((g) => g.models.length > 0)

    // Unlike `api_key`, a `cli_delegate` vendor the user hasn't connected
    // yet has no `providers` row at all — nothing for `remoteGroups`
    // above to find. Shown anyway, disabled, so it's discoverable
    // (spec.md FR-004, Acceptance Scenario 2); connecting happens from
    // Settings (tasks.md T027), not from this picker.
    const notConnectedDelegateGroups: ModelGroup[] = DELEGATE_VENDORS.filter(
      (vendor) =>
        !providerList.value.some(
          (p) =>
            p.kind === 'cli_delegate' &&
            p.adapter === vendor &&
            p.hasCredentials,
        ),
    ).map((vendor) => {
      const label = t(`chat.model.delegate.${vendor}`)
      return {
        providerId: `delegate-${vendor}`,
        providerName: label,
        models: [
          {
            id: `delegate-${vendor}:not-connected`,
            name: t('chat.model.delegateNotConnected'),
            disabled: true,
          },
        ],
      }
    })

    return localGroup
      ? [localGroup, ...remoteGroups, ...notConnectedDelegateGroups]
      : [...remoteGroups, ...notConnectedDelegateGroups]
  })

  /** Looks up a picker entry's friendly name across every group. */
  function findModelName(id: string): string {
    for (const group of modelGroups.value) {
      const found = group.models.find((m) => m.id === id)
      if (found) return found.name
    }
    return id
  }

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

  /**
   * Refreshes the provider list and re-fetches api_key/cli_delegate model
   * caches. Each provider's fetch is isolated: one provider being
   * unreachable (expired delegate token, network blip) must not blank out
   * every other provider's already-known models or abort the caller's
   * broader `initialize()` sequence.
   */
  async function refreshProviders() {
    providerList.value = await providers.listAsync()
    const relevant = providerList.value.filter(
      (p) => p.kind === 'api_key' || p.kind === 'cli_delegate',
    )
    const next: Record<string, ProviderModel[]> = {}
    const results = await Promise.allSettled(
      relevant.map(async (p) => {
        next[p.id] = await providers.listModelsAsync(p.id)
      }),
    )
    // A provider whose fetch failed this round keeps its last-known models
    // instead of being blanked out — `next` only ever holds currently
    // relevant providers, so one no longer connected still drops out below.
    for (const p of relevant) {
      if (!(p.id in next)) next[p.id] = providerModels.value[p.id] ?? []
    }
    const failure = results.find((r) => r.status === 'rejected') as
      PromiseRejectedResult | undefined
    if (failure) lastError.value = errString(failure.reason)
    providerModels.value = next
  }

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

  /** "Trotzdem als unsicher laden" — bypasses the hash check for this load only. */
  async function onIntegrityLoadUntrusted() {
    if (!integrityDialog.value) return
    const modelId = integrityDialog.value.modelId
    modelLoadPending.value = true
    const loadToken = beginLoadingModel(modelId, findModelName(modelId))
    integrityBusy.value = true
    integrityActionError.value = null
    try {
      activeModel.value =
        await chat.loadModelWithIntegrityOverrideAsync(modelId)
      integrityDialog.value = null
      try {
        await refreshInstalledAndCatalog()
      } catch (e) {
        // The override load itself already succeeded and the dialog is
        // gone by now — this failure has nowhere left to render as
        // `integrityActionError`, so it goes through the page's general
        // error banner instead of being silently dropped.
        lastError.value = errString(e)
      }
    } catch (e) {
      integrityActionError.value = errString(e)
    } finally {
      integrityBusy.value = false
      modelLoadPending.value = false
      clearLoadingModel(loadToken)
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
        try {
          await refreshInstalledAndCatalog()
        } catch (e) {
          // Same rationale as `onIntegrityLoadUntrusted`: the repair itself
          // already succeeded and the dialog is already gone, so a refresh
          // failure here goes through the general error banner instead of
          // a now-unreachable `integrityActionError`.
          lastError.value = errString(e)
        }
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
