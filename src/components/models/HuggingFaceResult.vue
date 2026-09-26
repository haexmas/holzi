<script setup lang="ts">
import type { HuggingFaceModelResult } from '~/composables/useHuggingFace'

const { t } = useI18n()

const props = defineProps<{
  result: HuggingFaceModelResult
}>()

const emit = defineEmits<{
  select: [result: HuggingFaceModelResult]
}>()

const catalogMatch = computed(() =>
  props.result.files.some((f) => f.catalogMatch),
)
</script>

<template>
  <button
    type="button"
    class="flex w-full flex-col gap-1 rounded-md px-3 py-3 text-left hover:bg-accent hover:text-accent-foreground focus:outline-none focus:ring-2 focus:ring-ring"
    @click="emit('select', result)"
  >
    <div class="flex items-center justify-between gap-2">
      <span class="font-medium">{{ result.displayName }}</span>
      <span
        v-if="catalogMatch"
        class="rounded bg-primary/10 px-1.5 py-0.5 text-xs text-primary"
      >
        {{ t('models.result.catalogMatch') }}
      </span>
    </div>
    <div class="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-muted-foreground">
      <span v-if="result.author"
        >{{ t('models.result.author') }}: {{ result.author }}</span
      >
      <span>{{ result.license ?? t('models.result.licenseUnknown') }}</span>
      <span v-if="result.downloads !== null"
        >{{ t('models.result.downloads') }}: {{ result.downloads }}</span
      >
    </div>
    <span class="text-xs text-muted-foreground">
      {{ t('models.result.filesCount', result.files.length) }}
    </span>
  </button>
</template>
