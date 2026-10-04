<script setup lang="ts">
/**
 * The attachments of an entry (spec 034, US5, FR-019..FR-021): add through the system's file dialog
 * or by dropping files on the list, rename, remove, download through the save dialog, and a preview
 * for images. Files travel as paths, never through the webview; a path is never stored. An
 * attachment is saved at once (it has no draft), so the editor can show this too. A preview is a
 * blob URL that is revoked when it closes or the list goes away.
 */
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open, save } from '@tauri-apps/plugin-dialog'
import { toast } from 'vue-sonner'
import type { AttachmentView } from '@bindings/AttachmentView'
import { formatFileSize, imageMime, safeFileName } from '~/lib/passwords/format'

const props = defineProps<{
  itemId: string
  attachments: AttachmentView[]
  /** An entry in the trash only shows its files. */
  readonly?: boolean
}>()

const emit = defineEmits<{
  changed: []
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const {
  attachmentAddAsync,
  attachmentRenameAsync,
  attachmentRemoveAsync,
  attachmentSaveAsync,
  attachmentPreviewAsync,
} = usePasswords()

const root = ref<HTMLElement | null>(null)
const busy = ref(false)
const dragging = ref(false)
const editing = ref<string | null>(null)
const fileName = ref('')
const previewId = ref<string | null>(null)
const previewUrl = ref<string | null>(null)

async function addPathsAsync(paths: string[]) {
  if (props.readonly || busy.value || !paths.length) return
  busy.value = true
  try {
    // One attachment per call: each is one write of its own.
    for (const path of paths) {
      try {
        await attachmentAddAsync(props.itemId, path)
      } catch (cause) {
        toast.error(errString(cause))
      }
    }
    emit('changed')
  } finally {
    busy.value = false
  }
}

async function pickAsync() {
  const selected = await open({ multiple: true })
  if (!selected) return
  await addPathsAsync(Array.isArray(selected) ? selected : [selected])
}

async function downloadAsync(attachment: AttachmentView) {
  const path = await save({ defaultPath: safeFileName(attachment.fileName) })
  if (!path) return
  try {
    await attachmentSaveAsync(attachment.id, path)
    toast.success(t('passwords.attachments.saved'))
  } catch (cause) {
    toast.error(errString(cause))
  }
}

function closePreview() {
  if (previewUrl.value) URL.revokeObjectURL(previewUrl.value)
  previewUrl.value = null
  previewId.value = null
}

async function togglePreviewAsync(attachment: AttachmentView) {
  if (previewId.value === attachment.id) {
    closePreview()
    return
  }
  const mime = imageMime(attachment.fileName)
  if (!mime) return
  try {
    const bytes = await attachmentPreviewAsync(attachment.id)
    closePreview()
    previewUrl.value = URL.createObjectURL(new Blob([bytes], { type: mime }))
    previewId.value = attachment.id
  } catch (cause) {
    toast.error(errString(cause))
  }
}

function startRename(attachment: AttachmentView) {
  editing.value = attachment.id
  fileName.value = attachment.fileName
}

async function saveRenameAsync(attachment: AttachmentView) {
  const name = fileName.value.trim()
  if (!name) return
  try {
    await attachmentRenameAsync(attachment.id, name)
    editing.value = null
    emit('changed')
  } catch (cause) {
    toast.error(errString(cause))
  }
}

async function removeAsync(attachment: AttachmentView) {
  if (previewId.value === attachment.id) closePreview()
  try {
    await attachmentRemoveAsync(attachment.id)
    emit('changed')
  } catch (cause) {
    toast.error(errString(cause))
  }
}

// Tauri hands dropped files over as paths, for the whole webview: only a drop whose position lies
// on this list counts.
let unlisten: (() => void) | undefined
function overList(position: { x: number; y: number }): boolean {
  const rect = root.value?.getBoundingClientRect()
  if (!rect) return false
  const x = position.x / window.devicePixelRatio
  const y = position.y / window.devicePixelRatio
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

onMounted(async () => {
  if (props.readonly) return
  try {
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload
      if (payload.type === 'leave') {
        dragging.value = false
      } else if (payload.type === 'drop') {
        dragging.value = false
        if (overList(payload.position)) void addPathsAsync(payload.paths)
      } else {
        dragging.value = overList(payload.position)
      }
    })
  } catch {
    // No webview (a preview in a browser): the add button still works.
  }
})

onBeforeUnmount(() => {
  unlisten?.()
  closePreview()
})
</script>

<template>
  <div ref="root" data-testid="passwords-attachments">
    <SettingsGroup :label="t('passwords.attachments.title')">
      <SettingsRow
        v-for="attachment in attachments"
        :key="attachment.id"
        :title="attachment.fileName"
        :description="formatFileSize(attachment.size)"
        icon="lucide:paperclip"
      >
        <template v-if="editing === attachment.id">
          <div class="w-48">
            <UiInput
              v-model="fileName"
              :aria-label="t('passwords.attachments.name')"
              :data-testid="`passwords-attachment-name-${attachment.id}`"
              @keydown.enter.prevent="saveRenameAsync(attachment)"
              @keydown.esc.prevent="editing = null"
            />
          </div>
          <UiButton
            type="button"
            size="sm"
            @click="saveRenameAsync(attachment)"
            >{{ t('passwords.save') }}</UiButton
          >
        </template>
        <template v-else>
          <UiButton
            v-if="imageMime(attachment.fileName)"
            type="button"
            variant="ghost"
            size="icon"
            :aria-label="t('passwords.attachments.preview')"
            :tooltip="t('passwords.attachments.preview')"
            :data-testid="`passwords-attachment-preview-${attachment.id}`"
            @click="togglePreviewAsync(attachment)"
          >
            <Icon
              :name="
                previewId === attachment.id ? 'lucide:eye-off' : 'lucide:eye'
              "
              class="size-4"
            />
          </UiButton>
          <UiButton
            type="button"
            variant="ghost"
            size="icon"
            :aria-label="t('passwords.attachments.download')"
            :tooltip="t('passwords.attachments.download')"
            :data-testid="`passwords-attachment-download-${attachment.id}`"
            @click="downloadAsync(attachment)"
          >
            <Icon name="lucide:download" class="size-4" />
          </UiButton>
          <template v-if="!readonly">
            <UiButton
              type="button"
              variant="ghost"
              size="icon"
              :aria-label="t('passwords.attachments.rename')"
              :tooltip="t('passwords.attachments.rename')"
              :data-testid="`passwords-attachment-rename-${attachment.id}`"
              @click="startRename(attachment)"
            >
              <Icon name="lucide:pencil" class="size-4" />
            </UiButton>
            <UiButton
              type="button"
              variant="ghost"
              size="icon"
              :aria-label="t('passwords.attachments.remove')"
              :tooltip="t('passwords.attachments.remove')"
              :data-testid="`passwords-attachment-remove-${attachment.id}`"
              @click="removeAsync(attachment)"
            >
              <Icon name="lucide:trash-2" class="size-4" />
            </UiButton>
          </template>
        </template>
        <template v-if="previewId === attachment.id && previewUrl" #below>
          <img
            :src="previewUrl"
            :alt="attachment.fileName"
            class="max-h-64 max-w-full rounded-lg object-contain"
            :data-testid="`passwords-attachment-image-${attachment.id}`"
          />
        </template>
      </SettingsRow>
      <SettingsRow
        v-if="!readonly"
        :class="dragging ? 'bg-foreground/10' : ''"
        :title="
          attachments.length
            ? t('passwords.attachments.addMore')
            : t('passwords.attachments.empty')
        "
        :description="t('passwords.attachments.hint')"
        icon="lucide:file-plus"
      >
        <UiButton
          type="button"
          size="sm"
          :disabled="busy"
          data-testid="passwords-attachment-add"
          @click="pickAsync"
        >
          {{ t('passwords.attachments.add') }}
        </UiButton>
      </SettingsRow>
    </SettingsGroup>
  </div>
</template>
