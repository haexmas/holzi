<script setup lang="ts">
import type { TierRecommendation, Tier } from '~/composables/useCatalog'
import type { SttCatalogEntry } from '~/composables/useSttCatalog'

const { t } = useI18n()

defineProps<{
  tiers: TierRecommendation<SttCatalogEntry>[]
  downloadingId: string | null
  downloadError: string | null
}>()

const emit = defineEmits<{
  choose: [tier: TierRecommendation<SttCatalogEntry>]
  skip: []
  back: []
}>()

function tierLabelKey(tier: Tier): string {
  return `onboarding.model.tier.${tier}`
}

function fitLabelKey(fit: string): string {
  return `onboarding.model.fit.${fit}`
}

/** Rough MB/GB display — good enough for three fixed catalog entries. */
function formatSize(bytes: number): string {
  const mb = bytes / (1024 * 1024)
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${Math.round(mb)} MB`
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <h2 class="text-xl font-semibold">
      {{ t('onboarding.sttModel.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('onboarding.sttModel.description') }}
    </p>

    <div v-if="tiers.length === 0" class="text-sm text-neutral-500">
      {{ t('onboarding.sttModel.empty') }}
    </div>

    <div v-else class="grid grid-cols-1 sm:grid-cols-3 gap-3">
      <button
        v-for="rec in tiers"
        :key="`${rec.tier}-${rec.entry.id}`"
        type="button"
        class="flex flex-col gap-2 rounded-md border border-neutral-300 p-3 text-left hover:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-progress"
        :disabled="downloadingId !== null"
        @click="emit('choose', rec)"
      >
        <span class="text-sm font-semibold uppercase tracking-wide">
          {{ t(tierLabelKey(rec.tier)) }}
        </span>
        <span class="text-base font-medium">{{ rec.entry.name }}</span>
        <span class="text-xs text-neutral-500">
          {{ formatSize(rec.entry.approxSizeBytes) }}
        </span>
        <span class="text-xs">
          {{ t(fitLabelKey(rec.fit)) }}
        </span>
      </button>
    </div>

    <p v-if="downloadingId" class="text-sm text-neutral-500" role="status">
      {{ t('onboarding.sttModel.downloading') }} ({{ downloadingId }})
    </p>
    <p v-if="downloadError" class="text-sm text-red-500" role="alert">
      {{ t('onboarding.sttModel.downloadFailed') }}: {{ downloadError }}
    </p>

    <div class="flex items-center justify-between">
      <UiButton
        variant="ghost"
        :disabled="downloadingId !== null"
        @click="emit('back')"
      >
        {{ t('onboarding.wizard.back') }}
      </UiButton>
      <UiButton
        variant="outline"
        :disabled="downloadingId !== null"
        @click="emit('skip')"
      >
        {{ t('onboarding.sttModel.skip') }}
      </UiButton>
    </div>
  </div>
</template>
