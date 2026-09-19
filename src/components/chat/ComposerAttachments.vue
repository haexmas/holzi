<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import type { AttachmentInfo } from '~/composables/useChat'

export interface ComposerAttachment {
  id: string
  path: string
  info: AttachmentInfo
}

/** Extensions `chat/attachments.rs::kind_and_media_type` actually
 * recognizes — kept in sync manually, same as any other UI-side mirror of
 * a backend capability list in this composer. */
const SUPPORTED_EXTENSIONS = [
  'png',
  'jpg',
  'jpeg',
  'webp',
  'gif',
  'pdf',
  'txt',
  'md',
]

const props = defineProps<{
  attachments: ComposerAttachment[]
  disabled?: boolean
}>()

const emit = defineEmits<{
  add: [paths: string[]]
  remove: [id: string]
}>()

const { t } = useI18n()

async function pickFiles() {
  if (props.disabled) return
  const selected = await open({
    multiple: true,
    filters: [
      {
        name: t('chat.composer.attachments.filterName'),
        extensions: SUPPORTED_EXTENSIONS,
      },
    ],
  })
  if (!selected) return
  emit('add', Array.isArray(selected) ? selected : [selected])
}
</script>

<template>
  <div class="flex min-w-0 shrink-0 items-center">
    <button
      type="button"
      class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground outline-none transition-colors hover:bg-muted/60 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
      :disabled="disabled"
      :aria-label="t('chat.composer.attachments.attachButton')"
      :title="t('chat.composer.attachments.attachButton')"
      @click="pickFiles"
    >
      <Icon name="lucide:plus" class="h-4 w-4" aria-hidden="true" />
    </button>

    <div
      v-if="attachments.length"
      class="flex min-w-0 items-center gap-1.5 overflow-x-auto"
    >
      <div
        v-for="attachment in attachments"
        :key="attachment.id"
        class="flex shrink-0 items-center gap-1 rounded-lg border border-border/70 bg-muted/20 py-1 pl-2 pr-1 text-xs"
        :class="{ 'text-destructive': !attachment.info.usable }"
        :title="attachment.info.reason ?? attachment.info.name"
      >
        <span class="max-w-[8rem] truncate">{{ attachment.info.name }}</span>
        <button
          type="button"
          class="flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-muted hover:text-foreground"
          :aria-label="t('chat.composer.attachments.remove', { name: attachment.info.name })"
          @click="emit('remove', attachment.id)"
        >
          <Icon name="lucide:x" class="h-3 w-3" aria-hidden="true" />
        </button>
      </div>
    </div>
  </div>
</template>
