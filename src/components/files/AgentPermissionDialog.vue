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
const requestTimeouts = new Map<string, ReturnType<typeof setTimeout>>()
const PERMISSION_REQUEST_TIMEOUT_MS = 60_000

const emit = defineEmits<{
  ready: []
}>()

function removeRequest(requestId: string): void {
  const timeout = requestTimeouts.get(requestId)
  if (timeout) clearTimeout(timeout)
  requestTimeouts.delete(requestId)
  queue.value = queue.value.filter((request) => request.requestId !== requestId)
}

onMounted(async () => {
  try {
    unlisten = await listen<FilesAgentPermissionRequest>(
      'files-agent-permission-request',
      (event) => {
        const request = event.payload
        queue.value.push(request)
        requestTimeouts.set(
          request.requestId,
          setTimeout(
            () => removeRequest(request.requestId),
            PERMISSION_REQUEST_TIMEOUT_MS,
          ),
        )
      },
    )
  } catch (error) {
    console.error(
      '[files] listening for agent permission requests failed',
      error,
    )
  } finally {
    emit('ready')
  }
})

onBeforeUnmount(() => {
  unlisten?.()
  for (const timeout of requestTimeouts.values()) clearTimeout(timeout)
  requestTimeouts.clear()
})

async function answerAsync(choice: FilesAgentChoice): Promise<void> {
  const request = queue.value.shift()
  if (!request) return
  const timeout = requestTimeouts.get(request.requestId)
  if (timeout) clearTimeout(timeout)
  requestTimeouts.delete(request.requestId)
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
