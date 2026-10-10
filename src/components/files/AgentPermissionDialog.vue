<script setup lang="ts">
/**
 * The question when the agent of the chat reaches for a storage it holds no permission for (spec
 * 044 FR-031a, research R14): read, read and write, or deny. Rust stores the answer; without an
 * answer in 60 s it refuses this one call. Questions that arrive together wait in line.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { FilesAgentChoice } from '@bindings/FilesAgentChoice'
import type { FilesAgentPermissionRequest } from '@bindings/FilesAgentPermissionRequest'

const { t } = useI18n()
const queue = ref<FilesAgentPermissionRequest[]>([])
const current = computed(() => queue.value[0])
let unlisten: UnlistenFn | undefined

onMounted(async () => {
  unlisten = await listen<FilesAgentPermissionRequest>(
    'files-agent-permission-request',
    (event) => {
      queue.value.push(event.payload)
    },
  )
})

onBeforeUnmount(() => unlisten?.())

async function answerAsync(choice: FilesAgentChoice): Promise<void> {
  const request = queue.value.shift()
  if (!request) return
  await invoke('files_agent_permission_answer', {
    args: { requestId: request.requestId, choice },
  }).catch((error: unknown) => {
    console.error('[files] answering the storage question failed', error)
  })
}
</script>

<template>
  <ShadcnAlertDialog
    :open="current !== undefined"
    @update:open="(open: boolean) => !open && answerAsync('deny')"
  >
    <ShadcnAlertDialogContent
      v-if="current"
      data-testid="files-agent-permission"
    >
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>
          {{ t('files.agentPermission.title') }}
        </ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription>
          {{
            t(`files.agentPermission.${current.wants}`, {
              name: current.storageName,
            })
          }}
          {{ t('files.agentPermission.hint') }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter class="flex-wrap gap-2">
        <ShadcnAlertDialogCancel data-testid="files-agent-permission-deny">
          {{ t('files.agentPermission.deny') }}
        </ShadcnAlertDialogCancel>
        <UiButton
          variant="outline"
          data-testid="files-agent-permission-read"
          @click="answerAsync('read')"
        >
          {{ t('files.agentPermission.allowRead') }}
        </UiButton>
        <UiButton
          data-testid="files-agent-permission-read-write"
          @click="answerAsync('readWrite')"
        >
          {{ t('files.agentPermission.allowReadWrite') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
