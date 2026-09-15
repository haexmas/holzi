<script setup lang="ts">
/**
 * The chat page's "no model loaded yet" states: downloading a catalog
 * model when none are installed, or picking an installed/provider model
 * once at least one is available — plus the pre-load integrity dialog
 * (`ModelsModelIntegrityDialog`) that either path can trigger.
 *
 * Extracted from `src/pages/chat/[instance].vue` (2026-09-15 review,
 * split step 4/4). Every ref that drives this UI (activeModel,
 * installedModels, catalogEntries, downloadingId, integrityDialog, ...)
 * stays on the page — this component is markup plus pure display
 * helpers, driven entirely through props/emits, so nothing the 19
 * `check-chat-state.ts` replay tests or the composer/send-flow depends
 * on moved with it.
 */
import type { CatalogEntryWithFit } from '~/composables/useCatalog'
import type { ModelIntegrityFailure } from '~/composables/useModels'

export type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string }[]
}

const { t } = useI18n()

defineProps<{
  noModelsInstalled: boolean
  catalogEntries: CatalogEntryWithFit[]
  downloadingId: string | null
  downloadProgressBytes: number
  downloadTotalBytes: number | null
  activeModelId: string
  busy: boolean
  modelGroups: ModelGroup[]
  integrityDialog: ModelIntegrityFailure | null
  integrityBusy: boolean
  integrityActionError: string | null
}>()

const emit = defineEmits<{
  downloadCatalogEntry: [entry: CatalogEntryWithFit]
  loadModel: [id: string]
  integrityDialogOpenChange: [open: boolean]
  integrityLoadUntrusted: []
  integrityRepairSource: []
  integrityChooseOther: []
}>()

function onModelPicked(event: Event) {
  emit('loadModel', (event.target as HTMLSelectElement).value)
}

/** Maps a hardware-fit verdict to its localized display label. */
function fitLabel(f: CatalogEntryWithFit['fit']): string {
  return t(`chat.fit.${f}`)
}

/** Formats a byte count for the model download UI. */
function humanBytes(n: number | null): string {
  if (n === null) return '?'
  const kb = 1024
  const mb = kb * 1024
  const gb = mb * 1024
  if (n >= gb) return (n / gb).toFixed(1) + ' GB'
  if (n >= mb) return (n / mb).toFixed(0) + ' MB'
  return (n / kb).toFixed(0) + ' KB'
}

function downloadProgressPercent(
  modelId: string,
  downloadingId: string | null,
  downloadTotalBytes: number | null,
  downloadProgressBytes: number,
): number | null {
  if (
    downloadingId !== modelId ||
    downloadTotalBytes === null ||
    downloadTotalBytes <= 0
  )
    return null
  return Math.min(
    100,
    Math.max(0, Math.round((downloadProgressBytes / downloadTotalBytes) * 100)),
  )
}
</script>

<template>
  <div v-if="noModelsInstalled" class="flex-1 overflow-y-auto p-6">
    <h2 class="text-lg font-semibold mb-4">
      {{ t('chat.empty.noModelsTitle') }}
    </h2>
    <p class="text-sm text-muted-foreground mb-6">
      {{ t('chat.empty.noModelsDescription') }}
    </p>
    <div class="space-y-2">
      <div
        v-for="e in catalogEntries"
        :key="e.id"
        class="relative overflow-hidden border border-border rounded p-3 flex items-center justify-between gap-4"
      >
        <div
          v-if="downloadingId === e.id"
          class="pointer-events-none absolute inset-y-0 left-0 bg-blue-100/70 transition-[width] duration-150"
          :class="
            downloadProgressPercent(
              e.id,
              downloadingId,
              downloadTotalBytes,
              downloadProgressBytes,
            ) === null
              ? 'animate-pulse'
              : ''
          "
          :style="{
            width: `${
              downloadProgressPercent(
                e.id,
                downloadingId,
                downloadTotalBytes,
                downloadProgressBytes,
              ) ?? 35
            }%`,
          }"
          role="progressbar"
          :aria-valuenow="
            downloadProgressPercent(
              e.id,
              downloadingId,
              downloadTotalBytes,
              downloadProgressBytes,
            ) ?? undefined
          "
          aria-valuemin="0"
          aria-valuemax="100"
          :aria-label="`${humanBytes(downloadProgressBytes)} / ${humanBytes(downloadTotalBytes)}`"
        />
        <div class="relative z-10 flex-1 min-w-0">
          <div class="font-medium text-sm">
            {{ e.name }}
          </div>
          <div class="text-xs text-muted-foreground truncate">
            {{ e.hf_repo }}/{{ e.hf_filename }}
          </div>
          <div class="text-xs text-muted-foreground">
            {{
              t('chat.catalog.meta', {
                size: humanBytes(e.approx_size_bytes),
                context: e.context_window.toLocaleString(),
                license: e.license,
                fit: fitLabel(e.fit),
              })
            }}
          </div>
        </div>
        <div class="relative z-10">
          <UiButton
            size="sm"
            :disabled="downloadingId !== null"
            @click="emit('downloadCatalogEntry', e)"
          >
            <template v-if="downloadingId === e.id">
              {{ humanBytes(downloadProgressBytes) }} /
              {{ humanBytes(downloadTotalBytes) }}
            </template>
            <template v-else>
              {{ t('chat.download') }}
            </template>
          </UiButton>
        </div>
      </div>
    </div>
  </div>

  <div
    v-else
    class="flex-1 flex items-center justify-center p-6 text-muted-foreground"
  >
    <div class="flex w-full max-w-sm flex-col gap-3">
      <p>{{ t('chat.model.selectPrompt') }}</p>
      <label for="chat-model-empty" class="sr-only">{{
        t('chat.model.label')
      }}</label>
      <select
        id="chat-model-empty"
        class="rounded-lg border border-border bg-background px-3 py-2 text-sm"
        :value="activeModelId"
        :disabled="busy"
        @change="onModelPicked"
      >
        <option value="" disabled>{{ t('chat.model.choose') }}</option>
        <optgroup
          v-for="group in modelGroups"
          :key="group.providerId"
          :label="group.providerName"
        >
          <option v-for="m in group.models" :key="m.id" :value="m.id">
            {{ m.name }}
          </option>
        </optgroup>
      </select>
    </div>
  </div>

  <ModelsModelIntegrityDialog
    v-if="integrityDialog"
    :open="integrityDialog !== null"
    :error-kind="integrityDialog.errorKind"
    :expected-sha256="integrityDialog.expected"
    :actual-sha256="integrityDialog.actual"
    :busy="integrityBusy"
    :action-error="integrityActionError"
    @update:open="emit('integrityDialogOpenChange', $event)"
    @load-untrusted="emit('integrityLoadUntrusted')"
    @repair-source="emit('integrityRepairSource')"
    @choose-other="emit('integrityChooseOther')"
  />
</template>
