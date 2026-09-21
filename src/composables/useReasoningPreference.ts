import { computed, ref, watch, type Ref } from 'vue'
import type {
  ModelCapabilities,
  ReasoningOption,
} from '~/composables/useModels'
import type { PrefScope, usePreferences } from '~/composables/usePreferences'

/**
 * How the composer's effort control presents itself for the displayed model
 * (spec 012, FR-005):
 * - `selectable`: the model offers options the user can pick from;
 * - `managed`: the model reasons on its own — shown disabled, labelled;
 * - `unknown`: a model row exists but its reasoning control is not
 *   determined yet — shown disabled, pointing at the provider refresh;
 * - `hidden`: nothing to choose (no reasoning control, or no model is
 *   resolved yet — never "not yet known" for a model nobody looked up).
 */
export type EffortState = 'selectable' | 'hidden' | 'managed' | 'unknown'

/** Device-scoped key holding one model's chosen option id; absent means Auto. */
const PREF_KEY_PREFIX = 'chat.reasoning_option.'
const prefKey = (modelId: string) => `${PREF_KEY_PREFIX}${modelId}`

export interface ReasoningPreferenceDeps {
  /** The displayed model's id (`''` when none). */
  modelId: Ref<string>
  /**
   * Its cached capabilities: `undefined` = no resolved row, `null` = a row
   * whose capabilities are not determined (see `capabilitiesFor`).
   */
  capabilities: Ref<ModelCapabilities | null | undefined>
  /**
   * This vault device's uuid, `null` until it is known. The preference is
   * device-scoped, so nothing is read or written before then.
   */
  deviceUuid: Ref<string | null>
  preferences: Pick<
    ReturnType<typeof usePreferences>,
    'getPrefAsync' | 'setPrefAsync' | 'clearPrefAsync'
  >
  errString: (error: unknown) => string
  /** Reports a failure through the owning store's error state. */
  setError: (message: string) => void
}

/**
 * The selected reasoning option for the displayed model, remembered per model
 * (per provider connection, since model ids are composite) and per device. It
 * is a user preference kept apart from the provider facts in the capability
 * record: refreshing a provider never overwrites a choice, it only clears one
 * the model no longer offers.
 */
export function useReasoningPreference(deps: ReasoningPreferenceDeps) {
  const {
    modelId,
    capabilities,
    deviceUuid,
    preferences,
    errString,
    setError,
  } = deps

  /** The effective option id for the displayed model; `null` = Auto. */
  const effortLevel = ref<string | null>(null)

  // The stored/chosen option for the displayed model, kept even while it
  // cannot be validated yet (the model's capabilities still loading), so a
  // late-arriving capability record can still apply it.
  let chosenOption: string | null = null
  // Monotonic guard: a read that finishes after the model changed, or after
  // the user already chose, must not overwrite the newer state.
  let loadToken = 0
  // Preference writes for one model must commit in call order. Different
  // models use independent queues, so switching models does not serialize
  // unrelated preference changes.
  const mutationQueues = new Map<string, Promise<void>>()

  const control = computed(() => capabilities.value?.reasoning ?? null)

  const effortOptions = computed<ReasoningOption[]>(() => {
    const current = control.value
    return current?.kind === 'presets' ? current.options : []
  })

  const effortState = computed<EffortState>(() => {
    if (capabilities.value === undefined) return 'hidden'
    const current = control.value
    if (current === null) return 'unknown'
    switch (current.kind) {
      case 'model_managed':
        return 'managed'
      case 'presets':
        return current.options.length > 0 ? 'selectable' : 'hidden'
      default:
        return 'hidden'
    }
  })

  function scope(): PrefScope | null {
    const uuid = deviceUuid.value
    return uuid ? { kind: 'device', uuid } : null
  }

  function enqueueMutation(
    forModel: string,
    mutation: () => Promise<void>,
  ): Promise<void> {
    const previous = mutationQueues.get(forModel) ?? Promise.resolve()
    const next = previous.catch(() => undefined).then(mutation)
    mutationQueues.set(forModel, next)
    void next
      .finally(() => {
        if (mutationQueues.get(forModel) === next) {
          mutationQueues.delete(forModel)
        }
      })
      .catch(() => undefined)
    return next
  }

  async function clearStored(forModel: string) {
    const target = scope()
    if (!target || !forModel) return
    await enqueueMutation(forModel, async () => {
      try {
        await preferences.clearPrefAsync(target, prefKey(forModel))
      } catch (e) {
        setError(errString(e))
      }
    })
  }

  /**
   * Derives the effective value from the chosen option and what the model
   * offers now. While the model's reasoning control cannot be determined the
   * choice is kept (Auto is shown); once it is determined and no longer
   * offers the option, the option is dropped and its stored key cleared.
   */
  async function reconcile() {
    const chosen = chosenOption
    const current = control.value
    if (chosen === null || current === null) {
      effortLevel.value = null
      return
    }
    if (
      current.kind === 'presets' &&
      current.options.some((o) => o.id === chosen)
    ) {
      effortLevel.value = chosen
      return
    }
    effortLevel.value = null
    chosenOption = null
    await clearStored(modelId.value)
  }

  async function loadStored(forModel: string) {
    const target = scope()
    if (!forModel || !target) return
    loadToken += 1
    const token = loadToken
    let stored: string | null
    try {
      stored = await preferences.getPrefAsync(target, prefKey(forModel))
    } catch (e) {
      setError(errString(e))
      return
    }
    // Superseded: the model changed or the user chose while this read ran.
    if (token !== loadToken || modelId.value !== forModel) return
    chosenOption = stored
    await reconcile()
  }

  /**
   * Accepts only Auto (`null`) or an option the model currently offers,
   * applies it at once and remembers it; if remembering fails the previous
   * value is restored and the failure is reported.
   */
  async function updateEffortLevel(optionId: string | null) {
    if (
      optionId !== null &&
      !effortOptions.value.some((option) => option.id === optionId)
    ) {
      return
    }
    const forModel = modelId.value
    const previousLevel = effortLevel.value
    const previousChosen = chosenOption
    loadToken += 1
    effortLevel.value = optionId
    chosenOption = optionId
    const target = scope()
    if (!forModel || !target) return
    try {
      await enqueueMutation(forModel, async () => {
        if (optionId === null) {
          await preferences.clearPrefAsync(target, prefKey(forModel))
        } else {
          await preferences.setPrefAsync(target, prefKey(forModel), optionId)
        }
      })
    } catch (e) {
      if (modelId.value === forModel && effortLevel.value === optionId) {
        effortLevel.value = previousLevel
        chosenOption = previousChosen
      }
      setError(errString(e))
    }
  }

  // A choice belongs to one model: switching starts on Auto and then loads
  // that model's own stored choice.
  watch(modelId, async (id) => {
    loadToken += 1
    chosenOption = null
    effortLevel.value = null
    await loadStored(id)
  })

  // The device becoming known (it resolves once, at store initialization)
  // unlocks the read for a model that was already displayed.
  watch(deviceUuid, async (uuid) => {
    if (uuid) await loadStored(modelId.value)
  })

  // A capability change re-validates the choice: a provider that dropped the
  // chosen option sends the model back to Auto and clears the stale key.
  watch(control, () => reconcile())

  return { effortLevel, effortOptions, effortState, updateEffortLevel }
}
