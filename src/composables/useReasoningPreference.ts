import { computed, ref, watch, type Ref } from 'vue'
import type {
  ModelCapabilities,
  ReasoningOption,
} from '~/composables/useModels'

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

export interface ReasoningPreferenceDeps {
  /** The displayed model's id (`''` when none). */
  modelId: Ref<string>
  /**
   * Its cached capabilities: `undefined` = no resolved row, `null` = a row
   * whose capabilities are not determined (see `capabilitiesFor`).
   */
  capabilities: Ref<ModelCapabilities | null | undefined>
}

/**
 * The selected reasoning option for the displayed model. It is a per-model
 * user preference, kept apart from the provider facts in the capability
 * record: refreshing a provider never overwrites a choice, it only clears one
 * the model no longer offers.
 */
export function useReasoningPreference(deps: ReasoningPreferenceDeps) {
  const { modelId, capabilities } = deps

  /** The effective option id for the displayed model; `null` = Auto. */
  const effortLevel = ref<string | null>(null)

  const effortOptions = computed<ReasoningOption[]>(() => {
    const control = capabilities.value?.reasoning
    return control?.kind === 'presets' ? control.options : []
  })

  const effortState = computed<EffortState>(() => {
    const caps = capabilities.value
    if (caps === undefined) return 'hidden'
    const control = caps?.reasoning ?? null
    if (control === null) return 'unknown'
    switch (control.kind) {
      case 'model_managed':
        return 'managed'
      case 'presets':
        return control.options.length > 0 ? 'selectable' : 'hidden'
      default:
        return 'hidden'
    }
  })

  /** Accepts only Auto (`null`) or an option the model currently offers. */
  function updateEffortLevel(optionId: string | null) {
    if (
      optionId !== null &&
      !effortOptions.value.some((option) => option.id === optionId)
    ) {
      return
    }
    effortLevel.value = optionId
  }

  // A choice belongs to one model: switching models starts on Auto rather
  // than carrying the previous model's option over.
  watch(modelId, () => {
    effortLevel.value = null
  })

  // A capability change that removes the selected option falls back to Auto.
  watch(effortOptions, (options) => {
    if (
      effortLevel.value !== null &&
      !options.some((option) => option.id === effortLevel.value)
    ) {
      effortLevel.value = null
    }
  })

  return { effortLevel, effortOptions, effortState, updateEffortLevel }
}
