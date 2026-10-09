<script setup lang="ts">
/**
 * Files and folders dragged in from the system (spec 044 FR-024): Tauri hands them over as paths
 * for the whole web view, so only a drop whose position lies on this area counts (pattern of
 * `components/passwords/Attachments.vue`). The drop is a copy into the open folder.
 */
import { getCurrentWebview } from '@tauri-apps/api/webview'

const props = defineProps<{ disabled: boolean }>()
const emit = defineEmits<{ drop: [paths: string[]] }>()

const { t } = useI18n()
const root = useTemplateRef<HTMLElement>('root')
const over = ref(false)

function onArea(position: { x: number; y: number }): boolean {
  const rect = root.value?.getBoundingClientRect()
  if (!rect) return false
  const x = position.x / window.devicePixelRatio
  const y = position.y / window.devicePixelRatio
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

let unlisten: (() => void) | undefined
onMounted(async () => {
  try {
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload
      if (payload.type === 'leave') {
        over.value = false
      } else if (payload.type === 'drop') {
        over.value = false
        if (!props.disabled && payload.paths.length && onArea(payload.position))
          emit('drop', payload.paths)
      } else if (payload.type === 'enter' || payload.type === 'over') {
        over.value = !props.disabled && onArea(payload.position)
      }
    })
  } catch {
    // No web view (a preview in a browser): copying through the menu still works.
  }
})
onBeforeUnmount(() => unlisten?.())
</script>

<template>
  <div ref="root" class="relative h-full min-h-0">
    <slot />
    <div
      v-if="over"
      class="pointer-events-none absolute inset-2 flex items-center justify-center rounded-lg border-2 border-dashed border-primary bg-primary/5 text-sm font-medium text-primary"
      data-testid="files-drop-hint"
    >
      {{ t('files.drop') }}
    </div>
  </div>
</template>
