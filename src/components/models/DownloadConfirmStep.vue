<script setup lang="ts">
/**
 * The question before every model download (spec 043 FR-028), on every platform: it names the
 * size and the free space where models are stored, and when the space is short it warns and the
 * button says "Trotzdem laden". Nothing loads before the person confirms.
 */
import { invoke } from '@tauri-apps/api/core'
import type { DownloadCheck } from '~/types/bindings/DownloadCheck'
import type { DownloadTarget } from '~/types/bindings/DownloadTarget'
import { humanBytes } from '~/lib/models/format'

const props = defineProps<{
  /** The model to load; the check runs whenever the question opens. */
  target: DownloadTarget | null
  /** The model's name as the list shows it. */
  name: string
}>()
const emit = defineEmits<{ confirm: [] }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()

const check = ref<DownloadCheck | null>(null)
const failed = ref(false)

// Keyed by the target's content: parents pass a new object on every render, and an equal one must
// not ask again. An answer that arrives after the question moved on to another model is dropped.
watch(
  () => [open.value, JSON.stringify(props.target)] as const,
  async ([isOpen], _previous, onCleanup) => {
    let stale = false
    onCleanup(() => (stale = true))
    check.value = null
    failed.value = false
    const target = props.target
    if (!isOpen || target === null) return
    try {
      const answer = await invoke<DownloadCheck>('model_download_check', {
        target,
      })
      if (!stale) check.value = answer
    } catch {
      // Without the check the size stays unknown; the person can still decide.
      if (!stale) failed.value = true
    }
  },
  { immediate: true },
)

const size = computed(() =>
  check.value?.sizeBytes == null
    ? t('models.downloadConfirm.unknown')
    : humanBytes(check.value.sizeBytes),
)
const free = computed(() =>
  check.value?.freeBytes == null
    ? t('models.downloadConfirm.unknown')
    : humanBytes(check.value.freeBytes),
)
const short = computed(() => check.value !== null && !check.value.fits)

function confirm() {
  // Before closing: closing may clear the target the parent still needs.
  emit('confirm')
  open.value = false
}
</script>

<template>
  <ShadcnAlertDialog
    :open="open"
    @update:open="(value: boolean) => (open = value)"
  >
    <ShadcnAlertDialogContent data-testid="models-download-confirm">
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>{{
          t('models.downloadConfirm.title', { name })
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription class="flex flex-col gap-1">
          <span data-testid="models-download-size">
            {{ t('models.downloadConfirm.size', { size }) }}
          </span>
          <span data-testid="models-download-free">
            {{ t('models.downloadConfirm.free', { free }) }}
          </span>
          <span
            v-if="short"
            class="text-destructive"
            role="alert"
            data-testid="models-download-short"
          >
            {{ t('models.downloadConfirm.short') }}
          </span>
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel data-testid="models-download-cancel">{{
          t('models.downloadConfirm.cancel')
        }}</ShadcnAlertDialogCancel>
        <UiButton
          type="button"
          :variant="short ? 'destructive' : 'default'"
          :disabled="check === null && !failed"
          data-testid="models-download-start"
          @click="confirm"
        >
          {{
            short
              ? t('models.downloadConfirm.startAnyway')
              : t('models.downloadConfirm.start')
          }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
