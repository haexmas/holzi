<script setup lang="ts">
import { hfErrorKey, useHuggingFace, type HuggingFaceModelResult } from '~/composables/useHuggingFace'

const { t } = useI18n()
const { searchAsync } = useHuggingFace()

const emit = defineEmits<{
  select: [result: HuggingFaceModelResult]
}>()

const query = ref('')
const results = ref<HuggingFaceModelResult[]>([])
const loading = ref(false)
const errorKey = ref<string | null>(null)
const hasSearched = ref(false)

const trimmedQuery = computed(() => query.value.trim())
const queryTooShort = computed(() => trimmedQuery.value.length > 0 && trimmedQuery.value.length < 2)

/**
 * Explicit submit only — no request fires on every keystroke (plan
 * §"Frontend-Modellverwaltung"). On failure the previous result list is
 * kept so a transient network error does not clear what the operator was
 * already looking at.
 */
async function onSubmit() {
  if (queryTooShort.value || trimmedQuery.value.length === 0 || loading.value) return
  loading.value = true
  errorKey.value = null
  try {
    results.value = await searchAsync(trimmedQuery.value)
    hasSearched.value = true
  }
  catch (e) {
    errorKey.value = hfErrorKey(e)
  }
  finally {
    loading.value = false
  }
}
</script>

<template>
  <section class="flex flex-col gap-3">
    <h2 class="text-lg font-semibold">
      {{ t('models.search.title') }}
    </h2>
    <form class="flex gap-2" @submit.prevent="onSubmit">
      <ShadcnInput
        v-model="query"
        :placeholder="t('models.search.placeholder')"
        :aria-label="t('models.search.title')"
        class="flex-1"
      />
      <UiButton type="submit" :loading="loading" :disabled="trimmedQuery.length === 0 || queryTooShort">
        {{ t('models.search.submit') }}
      </UiButton>
    </form>

    <p v-if="queryTooShort" class="text-xs text-neutral-500">
      {{ t('models.search.tooShort') }}
    </p>

    <p v-if="errorKey" class="flex items-center gap-2 text-sm text-red-500" role="alert">
      {{ t(errorKey) }}
      <button type="button" class="underline" @click="onSubmit">
        {{ t('models.search.retry') }}
      </button>
    </p>

    <p v-if="loading" class="text-sm text-neutral-500" role="status">
      {{ t('models.search.loading') }}
    </p>

    <p v-else-if="hasSearched && results.length === 0 && !errorKey" class="text-sm text-neutral-500">
      {{ t('models.search.empty') }}
    </p>

    <div v-if="results.length > 0" class="flex flex-col gap-2">
      <ModelsHuggingFaceResult
        v-for="result in results"
        :key="result.repoId"
        :result="result"
        @select="emit('select', $event)"
      />
    </div>
  </section>
</template>
