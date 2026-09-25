<script setup lang="ts">
/**
 * The "+" dropdown: every registered app, never empty (FR-033). Selecting a
 * singleton app that already has a tab somewhere activates it instead of
 * opening a second one — `wm.addTab` (via the `wm.tab.new` action,
 * spec 020) already implements that search
 * (tabs.ts, T033); this only labels those entries as already open (T035,
 * plan research R18). The teleported dropdown (z-50) always paints above
 * the windows: `wm/Desktop.vue` keeps their growing `stack` z-indices
 * inside an isolated stacking context.
 */
import { WM_APPS } from '~/lib/wm/apps'

const props = defineProps<{
  windowId: string
}>()

const wm = useWindowManagerStore()
const { t } = useI18n()

function isOpenElsewhere(appId: string, multiInstance: boolean): boolean {
  if (multiInstance) return false
  return wm.windows.some((w) => w.tabs.some((tab) => tab.appId === appId))
}

const newTab = useAction('wm.tab.new')

function select(appId: string) {
  void newTab({ windowId: props.windowId, appId })
}
</script>

<template>
  <ShadcnDropdownMenu>
    <ShadcnDropdownMenuTrigger as-child>
      <slot />
    </ShadcnDropdownMenuTrigger>
    <ShadcnDropdownMenuContent align="start">
      <ShadcnDropdownMenuItem
        v-for="app in WM_APPS"
        :key="app.id"
        class="gap-2"
        @select="select(app.id)"
      >
        <Icon :name="app.icon" class="h-4 w-4" :aria-hidden="true" />
        <span>{{ t(app.titleKey) }}</span>
        <span
          v-if="isOpenElsewhere(app.id, app.multiInstance)"
          class="ml-auto text-xs text-muted-foreground"
        >
          {{ t('wm.tabs.alreadyOpen') }}
        </span>
      </ShadcnDropdownMenuItem>
    </ShadcnDropdownMenuContent>
  </ShadcnDropdownMenu>
</template>
