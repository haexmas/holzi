import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  beginPaste as beginPasteOperation,
  cutIds,
  fillAblage,
  settlePaste,
  type Ablage,
  type AblageMode,
  type PasteOperation,
  type Target,
} from '~/lib/passwords/clipboard'

/**
 * The Ablage of entries and folders (spec 036, FR-013, FR-021, research R10): the logic is the pure
 * module `lib/passwords/clipboard.ts`, this store holds the state. It holds ids and the mode only,
 * never a title or a value, and it never touches the clipboard of the operating system. All windows
 * of the password manager share it (they share the webview); it is not restored with the session,
 * not synchronized, and it is dropped when the last window of the password manager closes
 * (`PasswordsApp.vue`). A change of vault ends the process (spec 013), and the Ablage with it.
 */
export const usePasswordsClipboardStore = defineStore(
  'passwordsClipboard',
  () => {
    const ablage = ref<Ablage>(null)
    let pending: PasteOperation | null = null

    const filled = computed(() => ablage.value !== null)
    const count = computed(() => ablage.value?.targets.length ?? 0)
    const mode = computed(() => ablage.value?.mode ?? null)
    const dimmed = computed(() => cutIds(ablage.value))

    function fill(targets: readonly Target[], next: AblageMode) {
      ablage.value = fillAblage(targets, next)
    }
    function beginPaste() {
      const operation = beginPasteOperation(ablage.value, pending)
      if (operation) pending = operation
      return operation
    }
    function settle(operation: PasteOperation, succeeded: boolean) {
      const next = settlePaste(ablage.value, pending, operation, succeeded)
      ablage.value = next.ablage
      pending = next.pending
    }
    function clear() {
      ablage.value = null
    }

    return {
      ablage,
      filled,
      count,
      mode,
      dimmed,
      fill,
      beginPaste,
      settle,
      clear,
    }
  },
)
