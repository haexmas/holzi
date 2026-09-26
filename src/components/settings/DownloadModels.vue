<script setup lang="ts">
/**
 * Download models (spec 005, moved to `/models/download` by spec 023, research R10): the
 * recommended catalog models with their download progress, and the way into the HuggingFace
 * search as a sub-view.
 */
import type { CatalogEntryWithFit } from '~/composables/useCatalog'

const { t } = useI18n()
const downloadCatalog = useActionOrThrow('settings.models.downloadCatalog')
const downloads = useModelDownloadsStore()
const modelStore = useModelsStore()
const { installedModels: installed, catalogEntries } = storeToRefs(modelStore)

const loading = ref(true)
const busyModelId = ref<string | null>(null)
const errorKey = ref<string | null>(null)
const errorDetail = ref<string | null>(null)

async function reloadAsync() {
  loading.value = true
  errorKey.value = null
  errorDetail.value = null
  try {
    await modelStore.refreshInstalledAndCatalog()
  } catch (e) {
    errorKey.value = hfErrorKey(e)
    errorDetail.value = hfErrorDetail(e)
  } finally {
    loading.value = false
  }
}

/** Downloads that are no catalog row, e.g. a HuggingFace file started from its repository. */
const otherDownloads = computed(() => {
  const catalogIds = new Set(catalogEntries.value.map((entry) => entry.id))
  return Object.keys(downloads.downloads).filter((id) => !catalogIds.has(id))
})

function isInstalled(entry: CatalogEntryWithFit): boolean {
  return installed.value.some((model) => model.id === entry.id)
}

async function downloadAsync(entry: CatalogEntryWithFit) {
  busyModelId.value = entry.id
  errorKey.value = null
  errorDetail.value = null
  try {
    await downloadCatalog({ entryId: entry.id })
    await reloadAsync()
  } catch (e) {
    errorKey.value = hfErrorKey(e)
    errorDetail.value = hfErrorDetail(e)
  } finally {
    busyModelId.value = null
    downloads.clearDownload(entry.id)
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-2">
    <div class="border-b border-border pb-2">
      <SettingsOverviewRow
        to="/models/download/search"
        icon="lucide:search"
        :title="t('settings.locations.models.download.search.title')"
        :description="
          t('settings.locations.models.download.search.description')
        "
        data-testid="settings-row-models.download.search"
      />
    </div>

    <template v-if="otherDownloads.length > 0">
      <span class="text-xs font-medium text-muted-foreground uppercase">
        {{ t('settings.downloadModels.running') }}
      </span>
      <div
        v-for="id in otherDownloads"
        :key="id"
        class="relative flex flex-col overflow-hidden border-b border-border px-3 py-3"
      >
        <ModelsDownloadBar :model-id="id" />
        <span class="relative z-10 font-mono text-sm">{{ id }}</span>
        <ModelsDownloadStatus class="relative z-10" :model-id="id" />
      </div>
    </template>

    <span
      v-if="otherDownloads.length > 0"
      class="text-xs font-medium text-muted-foreground uppercase"
    >
      {{ t('settings.downloadModels.recommended') }}
    </span>

    <p v-if="errorKey" class="text-sm text-destructive" role="alert">
      {{ t(errorKey) }}
      <span v-if="errorDetail" class="block text-xs">{{ errorDetail }}</span>
    </p>
    <div v-if="loading" class="text-sm text-muted-foreground">
      {{ t('models.search.loading') }}
    </div>

    <div
      v-for="entry in catalogEntries"
      :key="entry.id"
      class="relative flex items-center justify-between gap-2 overflow-hidden border-b border-border px-3 py-3 last:border-b-0"
    >
      <ModelsDownloadBar :model-id="entry.id" />
      <div class="relative z-10 flex flex-col">
        <span class="font-medium">{{ entry.name }}</span>
        <span class="text-xs text-muted-foreground"
          >{{ entry.parameters }} · {{ entry.quantization }} ·
          {{ t(`models.filePicker.fit.${entry.fit}`) }}</span
        >
        <ModelsDownloadStatus :model-id="entry.id" />
      </div>
      <div class="relative z-10">
        <UiButton
          type="button"
          size="sm"
          :disabled="isInstalled(entry)"
          :loading="busyModelId === entry.id"
          @click="downloadAsync(entry)"
        >
          {{
            isInstalled(entry)
              ? t('models.installed.active')
              : t('models.filePicker.install')
          }}
        </UiButton>
      </div>
    </div>
  </section>
</template>
