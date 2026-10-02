import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  emptySelection,
  isSelected,
  pruneSelection,
  selectAll,
  selectRange,
  toggle,
  type SelectionState,
} from '~/lib/passwords/selection'

/**
 * The entries the user selected in the list (spec 034, US2, FR-012): the logic is the pure module
 * `lib/passwords/selection.ts`, this store only holds the state for the list, the toolbar and the
 * sidebar drop targets. Selecting is local to the window and never persisted.
 */
export const usePasswordsSelectionStore = defineStore(
  'passwordsSelection',
  () => {
    const state = ref<SelectionState>(emptySelection())

    const ids = computed(() => state.value.selected)
    const count = computed(() => state.value.selected.length)
    const active = computed(() => count.value > 0)

    function isIdSelected(id: string): boolean {
      return isSelected(state.value, id)
    }
    function toggleId(id: string) {
      state.value = toggle(state.value, id)
    }
    function rangeTo(visible: readonly string[], id: string) {
      state.value = selectRange(state.value, visible, id)
    }
    function selectEvery(visible: readonly string[]) {
      state.value = selectAll(visible)
    }
    function clear() {
      state.value = emptySelection()
    }
    /** Forgets what no longer exists or no longer shows. */
    function keepOnly(existing: ReadonlySet<string>) {
      state.value = pruneSelection(state.value, existing)
    }

    return {
      ids,
      count,
      active,
      isIdSelected,
      toggleId,
      rangeTo,
      selectEvery,
      clear,
      keepOnly,
    }
  },
)
