<script setup lang="ts">
/**
 * HuggingFace search, the location `/models/download/search` (spec 005, spec 023 research R1).
 * The submitted term and the filters live in the tab location's query (`q`, `quant`, `size`,
 * `fit`, replaced in place), so back from a repository returns to the same results. A result opens
 * its repository as a new location, passing the files that matched the filters.
 */
import type {
  HardwareFit,
  HuggingFaceFileCandidate,
  HuggingFaceModelResult,
} from '~/composables/useHuggingFace'

const { t } = useI18n()
const router = useTabRouter()
const { searchAsync, detailsAsync } = useHuggingFace()

const initial = router.route.query
const query = ref(initial.q ?? '')
const results = ref<HuggingFaceModelResult[]>([])
const loading = ref(false)
const errorKey = ref<string | null>(null)
const hasSearched = ref(false)
const quantizationFilter = ref(initial.quant ?? 'all')
const sizeLimitFilter = ref(initial.size ?? 'all')
const fitFilter = ref<'all' | HardwareFit>(
  (initial.fit as HardwareFit | undefined) ?? 'all',
)

const fitOptions: HardwareFit[] = ['fits', 'tight', 'too_big', 'unknown']
const sizeLimitOptions = [2, 4, 8, 16]

const trimmedQuery = computed(() => query.value.trim())
const queryTooShort = computed(
  () => trimmedQuery.value.length > 0 && trimmedQuery.value.length < 2,
)

/** Loads repositories and enriches them with file-level size/fit metadata. */
async function loadResultsAsync(searchQuery?: string) {
  if (loading.value) return
  const key = searchQuery ?? ''
  const cached = cachedSearchResults(key)
  if (cached) {
    results.value = cached
    hasSearched.value = true
    return
  }
  loading.value = true
  errorKey.value = null
  try {
    const searchResults = await searchAsync(searchQuery)
    // The search endpoint intentionally stays cheap and returns repository
    // siblings without file sizes. Enrich each hit at the resolved search
    // revision so size and hardware filters use real GGUF metadata.
    results.value = await Promise.all(
      searchResults.map(async (result) => {
        try {
          return await detailsAsync(
            result.repoId,
            result.sourceRevision ?? undefined,
          )
        } catch {
          // A single repository's detail endpoint may disappear or be rate
          // limited after the search. Keep that hit usable with its normalized
          // search metadata; unknown-size/fit filters will handle it safely.
          return result
        }
      }),
    )
    rememberSearchResults(key, results.value)
    hasSearched.value = true
  } catch (e) {
    errorKey.value = hfErrorKey(e)
  } finally {
    loading.value = false
  }
}

/**
 * Explicit submit only — no request fires on every keystroke (plan
 * §"Frontend-Modellverwaltung"). On failure the previous result list is
 * kept so a transient network error does not clear what the operator was
 * already looking at.
 */
async function onSubmit() {
  if (queryTooShort.value || trimmedQuery.value.length === 0 || loading.value)
    return
  router.setQuery({ q: trimmedQuery.value })
  await loadResultsAsync(trimmedQuery.value)
}

async function retryAsync() {
  if (loading.value) return
  forgetSearchResults()
  if (trimmedQuery.value.length > 0) {
    await loadResultsAsync(trimmedQuery.value)
  } else {
    await loadResultsAsync()
  }
}

const availableQuantizations = computed(() => {
  const values = new Set<string>()
  for (const result of results.value) {
    for (const file of result.files) {
      values.add(file.quantization ?? 'unknown')
    }
  }
  return [...values].sort((a, b) => a.localeCompare(b))
})

watch(availableQuantizations, (quantizations) => {
  if (
    quantizationFilter.value !== 'all' &&
    !quantizations.includes(quantizationFilter.value)
  ) {
    quantizationFilter.value = 'all'
  }
})

function fileMatchesFilters(file: HuggingFaceFileCandidate): boolean {
  if (quantizationFilter.value !== 'all') {
    const quantization = file.quantization ?? 'unknown'
    if (quantization !== quantizationFilter.value) return false
  }
  if (fitFilter.value !== 'all' && file.fit !== fitFilter.value) return false
  if (sizeLimitFilter.value !== 'all') {
    const sizeBytes = file.sizeBytes
    const maxBytes = Number(sizeLimitFilter.value) * 1024 * 1024 * 1024
    if (sizeBytes === null || sizeBytes > maxBytes) return false
  }
  return true
}

const filteredResults = computed(() =>
  results.value
    .map((result) => ({
      ...result,
      files: result.files.filter(fileMatchesFilters),
    }))
    .filter((result) => result.files.length > 0),
)

watch(
  [quantizationFilter, sizeLimitFilter, fitFilter],
  ([quant, size, fit]) => {
    router.setQuery({
      quant: quant === 'all' ? null : quant,
      size: size === 'all' ? null : size,
      fit: fit === 'all' ? null : fit,
    })
  },
)

const filtersActive = computed(
  () =>
    quantizationFilter.value !== 'all' ||
    sizeLimitFilter.value !== 'all' ||
    fitFilter.value !== 'all',
)

/** Opens the repository as a new location; the files that matched the filters go along. */
function openResult(result: HuggingFaceModelResult) {
  const [owner = '', ...rest] = result.repoId.split('/')
  router.push({
    path: `/models/download/repo/${encodeURIComponent(owner)}/${encodeURIComponent(rest.join('/'))}`,
    query: filtersActive.value
      ? { files: result.files.map((file) => file.filename).join(',') }
      : {},
  })
}

onMounted(() => {
  void loadResultsAsync(router.route.query.q || undefined)
})
</script>

<template>
  <section class="flex flex-col gap-3">
    <form class="flex gap-2" @submit.prevent="onSubmit">
      <ShadcnInput
        v-model="query"
        :placeholder="t('models.search.placeholder')"
        :aria-label="t('models.search.title')"
        class="flex-1"
      />
      <UiButton
        type="submit"
        :loading="loading"
        :disabled="trimmedQuery.length === 0 || queryTooShort"
      >
        {{ t('models.search.submit') }}
      </UiButton>
    </form>

    <p v-if="queryTooShort" class="text-xs text-muted-foreground">
      {{ t('models.search.tooShort') }}
    </p>

    <p
      v-if="errorKey"
      class="flex items-center gap-2 text-sm text-destructive"
      role="alert"
    >
      {{ t(errorKey) }}
      <button type="button" class="underline" @click="retryAsync">
        {{ t('models.search.retry') }}
      </button>
    </p>

    <p v-if="loading" class="text-sm text-muted-foreground" role="status">
      {{ t('models.search.loading') }}
    </p>

    <p
      v-if="hasSearched && trimmedQuery.length === 0 && !errorKey"
      class="text-sm text-muted-foreground"
    >
      {{ t('models.search.top') }}
    </p>

    <div
      v-if="results.length > 0"
      class="flex flex-wrap items-end gap-3"
      :aria-label="t('models.search.filters.title')"
      role="group"
    >
      <label class="flex flex-col gap-1 text-xs">
        <span>{{ t('models.search.filters.quantization') }}</span>
        <select
          v-model="quantizationFilter"
          class="rounded-md border border-border bg-transparent px-2 py-1.5 text-sm"
        >
          <option value="all">{{ t('models.search.filters.all') }}</option>
          <option
            v-for="quantization in availableQuantizations"
            :key="quantization"
            :value="quantization"
          >
            {{
              quantization === 'unknown'
                ? t('models.search.filters.unknown')
                : quantization
            }}
          </option>
        </select>
      </label>
      <label class="flex flex-col gap-1 text-xs">
        <span>{{ t('models.search.filters.maxSize') }}</span>
        <select
          v-model="sizeLimitFilter"
          class="rounded-md border border-border bg-transparent px-2 py-1.5 text-sm"
        >
          <option value="all">{{ t('models.search.filters.all') }}</option>
          <option
            v-for="limit in sizeLimitOptions"
            :key="limit"
            :value="String(limit)"
          >
            {{ t('models.search.filters.maxSizeValue', { size: limit }) }}
          </option>
        </select>
      </label>
      <label class="flex flex-col gap-1 text-xs">
        <span>{{ t('models.search.filters.fit') }}</span>
        <select
          v-model="fitFilter"
          class="rounded-md border border-border bg-transparent px-2 py-1.5 text-sm"
        >
          <option value="all">{{ t('models.search.filters.all') }}</option>
          <option v-for="fit in fitOptions" :key="fit" :value="fit">
            {{ t(`models.search.filters.fitValues.${fit}`) }}
          </option>
        </select>
      </label>
    </div>

    <p
      v-else-if="hasSearched && results.length === 0 && !errorKey"
      class="text-sm text-muted-foreground"
    >
      {{ t('models.search.empty') }}
    </p>

    <p
      v-if="hasSearched && results.length > 0 && filteredResults.length === 0"
      class="text-sm text-muted-foreground"
    >
      {{ t('models.search.filters.empty') }}
    </p>

    <div
      v-if="filteredResults.length > 0"
      class="flex flex-col divide-y divide-border"
    >
      <ModelsHuggingFaceResult
        v-for="result in filteredResults"
        :key="result.repoId"
        :result="result"
        @select="openResult"
      />
    </div>
  </section>
</template>
