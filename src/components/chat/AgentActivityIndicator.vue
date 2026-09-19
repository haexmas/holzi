<script setup lang="ts">
/**
 * Live "N agents" pill for Claude Code delegate sub-agent activity (spec
 * 011-composer-toolbar-parity, Story 2). Renders nothing while `count` is
 * 0 — never shown for a response with no sub-agents, or for a backend
 * with no sub-agent concept at all (FR-010).
 */
const props = defineProps<{
  count: number
  /** Size of the most recently dispatched batch, if any (FR-008) — used
   * only to enrich the tooltip/aria-label, not the visible label itself. */
  lastBatchSize?: number | null
}>()

const { t } = useI18n()

const title = computed(() =>
  props.lastBatchSize && props.lastBatchSize > 1
    ? t('chat.agentActivity.batchTitle', props.lastBatchSize)
    : undefined,
)
</script>

<template>
  <div
    v-if="count > 0"
    class="flex shrink-0 items-center gap-1.5 rounded-xl border border-border/70 bg-muted/20 px-2 py-1 text-xs font-medium text-foreground/80"
    role="status"
    :title="title"
  >
    <span
      class="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500"
      aria-hidden="true"
    />
    {{ t('chat.agentActivity.count', count) }}
  </div>
</template>
