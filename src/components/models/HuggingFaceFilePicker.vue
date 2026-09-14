<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  hfErrorKey,
  hfErrorDetail,
  useHuggingFace,
  type HuggingFaceFileCandidate,
  type HuggingFaceModelResult,
  type InstallPreview,
} from '~/composables/useHuggingFace'
import { useModels, type InstalledModel } from '~/composables/useModels'

const { t } = useI18n()
const { detailsAsync, previewInstallAsync } = useHuggingFace()
const { downloadFromHfAsync, onDownloadProgress, onDownloadComplete } = useModels()

const props = defineProps<{
  repoId: string
}>()

const emit = defineEmits<{
  installed: [model: InstalledModel]
  back: []
}>()

const details = ref<HuggingFaceModelResult | null>(null)
const detailsErrorKey = ref<string | null>(null)
const loadingDetails = ref(false)

const selectedFile = ref<HuggingFaceFileCandidate | null>(null)
const preview = ref<InstallPreview | null>(null)
const previewErrorKey = ref<string | null>(null)
const loadingPreview = ref(false)

const tokenizerRepoInput = ref('')
const tooBigConfirmed = ref(false)

const installing = ref(false)
const installErrorKey = ref<string | null>(null)
const installErrorDetail = ref<string | null>(null)
const downloadedBytes = ref(0)
const downloadTotalBytes = ref<number | null>(null)

let unlistenProgress: UnlistenFn | null = null
let unlistenComplete: UnlistenFn | null = null

async function loadDetailsAsync() {
  loadingDetails.value = true
  detailsErrorKey.value = null
  try {
    details.value = await detailsAsync(props.repoId)
  }
  catch (e) {
    detailsErrorKey.value = hfErrorKey(e)
  }
  finally {
    loadingDetails.value = false
  }
}

async function selectFileAsync(file: HuggingFaceFileCandidate) {
  selectedFile.value = file
  preview.value = null
  previewErrorKey.value = null
  tokenizerRepoInput.value = ''
  tooBigConfirmed.value = false
  loadingPreview.value = true
  try {
    preview.value = await previewInstallAsync({
      repoId: file.repoId,
      filename: file.filename,
      // Details exposes the resolved source SHA. Pass only a real tracked
      // ref here; otherwise preview would turn the default `main` tracking
      // into an untracked direct SHA pin and update checks would disappear.
      revision: file.revisionRef ?? undefined,
    })
  }
  catch (e) {
    previewErrorKey.value = hfErrorKey(e)
  }
  finally {
    loadingPreview.value = false
  }
}

const effectiveTokenizerRepo = computed(
  () => preview.value?.tokenizerRepo ?? (tokenizerRepoInput.value.trim() || null),
)
const needsTooBigConfirmation = computed(() => preview.value?.requiresExplicitTooBigConfirmation ?? false)
const canInstall = computed(() =>
  preview.value !== null
  && effectiveTokenizerRepo.value !== null
  && (!needsTooBigConfirmation.value || tooBigConfirmed.value)
  && !installing.value,
)

const downloadPercent = computed(() => {
  if (downloadTotalBytes.value === null || downloadTotalBytes.value <= 0) return null
  return Math.min(100, Math.max(0, Math.round((downloadedBytes.value / downloadTotalBytes.value) * 100)))
})

async function installAsync() {
  if (!preview.value || !selectedFile.value || !canInstall.value) return
  installing.value = true
  installErrorKey.value = null
  installErrorDetail.value = null
  downloadedBytes.value = 0
  downloadTotalBytes.value = preview.value.sizeBytes
  try {
    const model = await downloadFromHfAsync({
      repoId: selectedFile.value.repoId,
      filename: selectedFile.value.filename,
      // Preserve the preview's update semantics: tracked refs remain
      // tracked, while a deliberate direct SHA remains pinned.
      revision: preview.value.revisionRef ?? preview.value.revision,
      name: preview.value.name,
      tokenizerRepo: effectiveTokenizerRepo.value ?? undefined,
      contextWindow: preview.value.contextWindow,
      forceTooBig: tooBigConfirmed.value,
    })
    emit('installed', model)
  }
  catch (e) {
    installErrorKey.value = hfErrorKey(e)
    installErrorDetail.value = hfErrorDetail(e)
  }
  finally {
    installing.value = false
  }
}

function humanBytes(n: number | null): string {
  if (n === null) return t('models.filePicker.sizeUnknown')
  const kb = 1024
  const mb = kb * 1024
  const gb = mb * 1024
  if (n >= gb) return `${(n / gb).toFixed(1)} GB`
  if (n >= mb) return `${(n / mb).toFixed(0)} MB`
  return `${(n / kb).toFixed(0)} KB`
}

onMounted(async () => {
  await loadDetailsAsync()
  unlistenProgress = await onDownloadProgress((e) => {
    if (e.modelId !== preview.value?.modelId) return
    downloadedBytes.value = e.bytesDownloaded
    if (e.bytesTotal !== null) downloadTotalBytes.value = e.bytesTotal
  })
  unlistenComplete = await onDownloadComplete((model) => {
    if (model.id !== preview.value?.modelId) return
    downloadedBytes.value = downloadTotalBytes.value ?? downloadedBytes.value
  })
})

onBeforeUnmount(() => {
  unlistenProgress?.()
  unlistenComplete?.()
})
</script>

<template>
  <section class="flex flex-col gap-3">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">
        {{ t('models.filePicker.title') }}
      </h2>
      <UiButton variant="ghost" type="button" @click="emit('back')">
        {{ t('onboarding.wizard.back') }}
      </UiButton>
    </div>

    <p v-if="loadingDetails" class="text-sm text-neutral-500" role="status">
      {{ t('models.search.loading') }}
    </p>
    <p v-if="detailsErrorKey" class="text-sm text-red-500" role="alert">
      {{ t(detailsErrorKey) }}
    </p>
    <p v-if="details && details.files.length === 0" class="text-sm text-neutral-500">
      {{ t('models.result.noGgufFiles') }}
    </p>

    <div v-if="details" class="flex flex-col gap-2">
      <button
        v-for="file in details.files"
        :key="file.filename"
        type="button"
        class="flex flex-col gap-1 rounded-md border p-3 text-left focus:outline-none focus:ring-2 focus:ring-blue-500"
        :class="selectedFile?.filename === file.filename ? 'border-blue-500' : 'border-neutral-300 hover:border-blue-500'"
        @click="selectFileAsync(file)"
      >
        <span class="font-mono text-sm">{{ file.filename }}</span>
        <div class="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-neutral-500">
          <span>{{ t('models.filePicker.size') }}: {{ humanBytes(file.sizeBytes) }}</span>
          <span>{{ t('models.filePicker.quantization') }}: {{ file.quantization ?? t('models.filePicker.quantizationUnknown') }}</span>
          <span>{{ t(`models.filePicker.fit.${file.fit}`) }}</span>
          <span v-if="file.catalogMatch">{{ t('models.result.catalogMatch') }}</span>
        </div>
      </button>
    </div>

    <div v-if="selectedFile" class="relative flex flex-col gap-3 overflow-hidden rounded-md border border-neutral-300 p-3">
      <div
        v-if="installing"
        class="pointer-events-none absolute inset-y-0 left-0 bg-blue-100/70 transition-[width] duration-150"
        :class="downloadPercent === null ? 'animate-pulse' : ''"
        :style="{ width: `${downloadPercent ?? 35}%` }"
        role="progressbar"
        :aria-valuenow="downloadPercent ?? undefined"
        aria-valuemin="0"
        aria-valuemax="100"
        :aria-label="t('models.filePicker.downloadProgress', { done: humanBytes(downloadedBytes), total: humanBytes(downloadTotalBytes) })"
      />
      <div class="relative z-10 flex flex-col gap-3">
        <p v-if="loadingPreview" class="text-sm text-neutral-500" role="status">
          {{ t('models.search.loading') }}
        </p>
        <p v-if="previewErrorKey" class="text-sm text-red-500" role="alert">
          {{ t(previewErrorKey) }}
        </p>

        <template v-if="preview">
          <dl class="grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
            <dt class="text-neutral-500">
              {{ t('models.filePicker.size') }}
            </dt>
            <dd>{{ humanBytes(preview.sizeBytes) }}</dd>
            <dt class="text-neutral-500">
              {{ t('models.filePicker.quantization') }}
            </dt>
            <dd>{{ preview.quantization ?? t('models.filePicker.quantizationUnknown') }}</dd>
            <dt class="text-neutral-500">
              {{ t('models.filePicker.contextWindow') }}
            </dt>
            <dd>{{ preview.contextWindow ?? t('models.filePicker.contextWindowUnknown') }}</dd>
            <dt class="text-neutral-500">
              {{ t('models.filePicker.revision') }}
            </dt>
            <dd class="truncate font-mono">
              {{ preview.revision }}
            </dd>
            <template v-if="preview.revisionRef">
              <dt class="text-neutral-500">
                {{ t('models.filePicker.revisionRef') }}
              </dt>
              <dd>{{ preview.revisionRef }}</dd>
            </template>
          </dl>

          <div v-if="preview.tokenizerRequired" class="flex flex-col gap-1">
            <p class="text-xs text-amber-600">
              {{ t('models.filePicker.tokenizerHint') }}
            </p>
            <label class="flex flex-col gap-1">
              <span class="text-sm font-medium">{{ t('models.filePicker.tokenizerRepoLabel') }}</span>
              <ShadcnInput v-model="tokenizerRepoInput" :placeholder="t('models.filePicker.tokenizerRepoPlaceholder')" />
            </label>
          </div>

          <div v-if="needsTooBigConfirmation" class="flex flex-col gap-1">
            <p class="text-sm text-red-500" role="alert">
              {{ t('models.filePicker.tooBigWarning') }}
            </p>
            <label class="flex items-center gap-2 text-sm">
              <input v-model="tooBigConfirmed" type="checkbox">
              {{ t('models.filePicker.tooBigConfirm') }}
            </label>
          </div>

          <p v-if="installErrorKey" class="text-sm text-red-500" role="alert">
            {{ t(installErrorKey) }}
            <span v-if="installErrorDetail" class="block text-xs">{{ installErrorDetail }}</span>
          </p>
          <p v-if="installing" class="text-sm text-neutral-500" role="status">
            {{ t('models.filePicker.downloadProgress', { done: humanBytes(downloadedBytes), total: humanBytes(downloadTotalBytes) }) }}
          </p>

          <UiButton type="button" :disabled="!canInstall" :loading="installing" @click="installAsync">
            {{ t('models.filePicker.install') }}
          </UiButton>
        </template>
      </div>
    </div>
  </section>
</template>
