<script setup lang="ts">
/**
 * The permanent mark of a tab that shows a development version of an extension (spec 017, US12,
 * FR-064): it runs unsigned from a development server.
 */
const { t } = useI18n()
</script>

<template>
  <span
    class="shrink-0 rounded bg-warning/20 px-1 text-[10px] font-semibold tracking-wide text-foreground uppercase"
    :title="t('extensions.dev.badgeHint')"
    data-testid="dev-badge"
    >{{ t('extensions.dev.badge') }}</span
  >
</template>
