<script setup lang="ts">
/**
 * Model preload/readiness status, visible independent of any open window
 * (spec 015-workspace-shell, T025, FR-005). Markup moved unchanged from the
 * former workspace stub page; the listener wiring itself lives in
 * `useModelPreloadStatus` (T020).
 */
const chat = useChat()
const { t } = useI18n()
const { preloadStatus, preloadError } = useModelPreloadStatus(chat)
</script>

<template>
  <p
    v-if="preloadStatus?.status === 'loading'"
    class="flex items-center gap-2 px-4 py-1.5 text-xs text-muted-foreground"
    role="status"
  >
    <Icon
      name="lucide:loader-circle"
      class="h-3.5 w-3.5 animate-spin"
      :aria-hidden="true"
    />
    {{
      t('workspace.modelPreload.loading', {
        modelName: preloadStatus.modelName,
      })
    }}
  </p>
  <p
    v-else-if="preloadStatus?.status === 'ready'"
    class="flex items-center gap-2 px-4 py-1.5 text-xs text-muted-foreground"
    role="status"
  >
    <Icon
      name="lucide:check-circle-2"
      class="h-3.5 w-3.5 text-emerald-600"
      :aria-hidden="true"
    />
    {{
      t('workspace.modelPreload.ready', {
        modelName: preloadStatus.modelName,
      })
    }}
  </p>
  <p
    v-else-if="preloadError"
    class="flex items-center gap-2 px-4 py-1.5 text-xs text-muted-foreground"
    role="status"
  >
    <Icon
      name="lucide:circle-alert"
      class="h-3.5 w-3.5 text-amber-600"
      :aria-hidden="true"
    />
    {{ t('workspace.modelPreload.error') }}
  </p>
</template>
