import { ref, watch, type Ref } from 'vue'
import type { useChat } from '~/composables/useChat'
import type { ComposerAttachment } from '~/components/chat/ComposerAttachments.vue'

/**
 * Composer attachment list — extracted from `src/pages/chat/[instance].vue`
 * (spec 015-workspace-shell, T008, plan research R10). Owns `attachments`
 * itself (unlike `useComposer`, nothing else needs it to exist before this
 * composable runs) and re-evaluates it whenever the displayed model changes.
 */
export function useComposerAttachments(
  chat: ReturnType<typeof useChat>,
  displayModelId: Ref<string>,
  errString: (e: unknown) => string,
  lastError: Ref<string | null>,
) {
  const attachments = ref<ComposerAttachment[]>([])

  /** Adds newly picked files to the composer's attachment list, classifying
   * each against the displayed model/backend (spec 011-composer-toolbar-parity
   * Story 3). Duplicate paths are allowed — each gets its own entry (spec.md
   * Edge Cases). */
  async function addAttachments(paths: string[]) {
    for (const path of paths) {
      try {
        const info = await chat.inspectAttachmentAsync(
          path,
          displayModelId.value,
        )
        attachments.value.push({ id: crypto.randomUUID(), path, info })
      } catch (e: unknown) {
        lastError.value = errString(e)
      }
    }
  }

  function removeAttachment(id: string) {
    attachments.value = attachments.value.filter((a) => a.id !== id)
  }

  /** Re-evaluates staged attachments against the newly displayed model/backend
   * after a model switch (spec.md Edge Cases). */
  async function refreshAttachmentUsability() {
    const modelId = displayModelId.value
    await Promise.all(
      attachments.value.map(async (attachment) => {
        try {
          attachment.info = await chat.inspectAttachmentAsync(
            attachment.path,
            modelId,
          )
        } catch {
          // The file may have disappeared since it was attached — leave its
          // last-known info in place; send-time re-validation handles exclusion.
        }
      }),
    )
  }

  watch(displayModelId, refreshAttachmentUsability)

  return { attachments, addAttachments, removeAttachment }
}
