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
  <SettingsRow navigates @select="emit('select', result)">
    <template #title>
      <span class="flex flex-wrap items-center gap-2">
        <span class="font-medium">{{ result.displayName }}</span>
        <span
          v-if="catalogMatch"
          class="rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary"
        >
          {{ t('models.result.catalogMatch') }}
        </span>
      </span>
    </template>
    <template #description>
      <span class="flex flex-wrap gap-x-3 gap-y-0.5 text-xs">
        <span v-if="result.author"
          >{{ t('models.result.author') }}: {{ result.author }}</span
        >
        <span>{{ result.license ?? t('models.result.licenseUnknown') }}</span>
        <span v-if="result.downloads !== null"
          >{{ t('models.result.downloads') }}: {{ result.downloads }}</span
        >
        <span>{{ t('models.result.filesCount', result.files.length) }}</span>
      </span>
    </template>
  </SettingsRow>
</template>
