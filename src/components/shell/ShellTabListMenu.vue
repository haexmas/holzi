<script setup lang="ts">
/**
 * The Chevron dropdown (FR-034): every tab of the window, in bar order,
 * with icon, title, a checkmark on the active one, and an attention badge —
 * selecting one activates it (`ShellTabBar.vue`'s own watch then scrolls it
 * into view, FR-035). Spec 015-workspace-shell, T036, plan research R18.
 */
import { computed } from 'vue'
import type { ShellTab } from '~/lib/shell/types'

const props = defineProps<{
  tabs: ShellTab[]
  activeTabId: string
}>()

const emit = defineEmits<{
  selectTab: [tabId: string]
}>()

const shell = useShellStore()
const { t } = useI18n()

function titleFrom(info: ReturnType<typeof shell.tabDisplayInfo>): string {
  return info.titleOverride ?? (info.titleKey ? t(info.titleKey) : '')
}

const rows = computed(() =>
  props.tabs.map((tab) => ({ tab, info: shell.tabDisplayInfo(tab) })),
)
</script>

<template>
  <ShadcnDropdownMenu>
    <ShadcnDropdownMenuTrigger as-child>
      <button
        type="button"
        class="shrink-0 rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        :aria-label="t('shell.tabs.listMenu')"
        @pointerdown.stop
      >
        <Icon
          name="lucide:chevron-down"
          class="h-3.5 w-3.5"
          :aria-hidden="true"
        />
      </button>
    </ShadcnDropdownMenuTrigger>
    <ShadcnDropdownMenuContent align="end">
      <ShadcnDropdownMenuItem
        v-for="row in rows"
        :key="row.tab.id"
        class="gap-2"
        @select="emit('selectTab', row.tab.id)"
      >
        <Icon
          v-if="row.info.icon"
          :name="row.info.icon"
          class="h-4 w-4 shrink-0"
          :aria-hidden="true"
        />
        <span class="min-w-0 max-w-48 truncate">{{ titleFrom(row.info) }}</span>
        <Icon
          v-if="row.tab.id === activeTabId"
          name="lucide:check"
          class="h-3.5 w-3.5 shrink-0"
          :aria-hidden="true"
        />
        <span
          v-if="row.info.hasAttention"
          class="ml-auto h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500"
          :aria-label="t('shell.attention')"
        />
      </ShadcnDropdownMenuItem>
    </ShadcnDropdownMenuContent>
  </ShadcnDropdownMenu>
</template>
