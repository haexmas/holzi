<script setup lang="ts">
/**
 * The "+" dropdown: every registered app, never empty (FR-033). Selecting a
 * singleton app that already has a tab somewhere activates it instead of
 * opening a second one — `shell.addTab` already implements that search
 * (tabs.ts, T033); this only labels those entries as already open (T035,
 * plan research R18).
 *
 * Known limitation: the dropdown's stacking context (z-50, from the haex-ui
 * layer) is a fixed value, while `ShellWindow.vue` gives each window a
 * `z-index` from its `stack` rank, which only ever grows (`nextStack` is
 * never renumbered dense the way workspace `position` is). After roughly 50
 * focus changes in one session, a focused window could in principle render
 * above this menu. Not worth a stack-renumbering change for this task.
 */
import { SHELL_APPS } from '~/lib/shell/apps'

const props = defineProps<{
  windowId: string
}>()

const shell = useShellStore()
const { t } = useI18n()

function isOpenElsewhere(appId: string, multiInstance: boolean): boolean {
  if (multiInstance) return false
  return shell.windows.some((w) => w.tabs.some((tab) => tab.appId === appId))
}

function select(appId: string) {
  shell.addTab(props.windowId, appId)
}
</script>

<template>
  <ShadcnDropdownMenu>
    <ShadcnDropdownMenuTrigger as-child>
      <slot />
    </ShadcnDropdownMenuTrigger>
    <ShadcnDropdownMenuContent align="start">
      <ShadcnDropdownMenuItem
        v-for="app in SHELL_APPS"
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
          {{ t('shell.tabs.alreadyOpen') }}
        </span>
      </ShadcnDropdownMenuItem>
    </ShadcnDropdownMenuContent>
  </ShadcnDropdownMenu>
</template>
