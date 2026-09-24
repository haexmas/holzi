<script setup lang="ts">
/**
 * One Shell window: frame, minimal title bar (icon, title, close), and its
 * app content (spec 015-workspace-shell, T023). Deliberately minimal for
 * User Story 1 — drag, resize, minimize/maximize and the full Firefox-style
 * tab bar (FR-007, FR-031-038) are User Story 2/3 and extend this same
 * file (T029, T034-T037), not a replacement of it.
 *
 * Provides the `useShellTab()` contract for the window's one tab (Phase 3
 * has no multi-tab windows yet — that is tabs.ts, T033).
 */
import { computed } from 'vue'
import { getAppDefinition } from '~/lib/shell/apps'
import { getAppComponent } from '~/components/shell/appComponents'
import { provideShellTab } from '~/composables/useShellTab'
import type { ShellWindow } from '~/lib/shell/types'

const props = defineProps<{
  window: ShellWindow
  active: boolean
}>()

const shell = useShellStore()
const { t } = useI18n()

const tab = computed(() => props.window.tabs[0])
const app = computed(() =>
  tab.value ? getAppDefinition(tab.value.appId) : undefined,
)
const component = computed(() =>
  tab.value ? getAppComponent(tab.value.appId) : undefined,
)
const title = computed(() => {
  const runtime = tab.value ? shell.runtimeFor(tab.value.id) : null
  if (runtime?.titleOverride) return runtime.titleOverride
  return app.value ? t(app.value.titleKey) : ''
})

provideShellTab({
  get tabId() {
    return tab.value?.id ?? ''
  },
  get windowId() {
    return props.window.id
  },
  requestAttention: () => {
    if (tab.value) shell.setTabAttention(tab.value.id, true)
  },
  clearAttention: () => {
    if (tab.value) shell.setTabAttention(tab.value.id, false)
  },
  setTitle: (value) => {
    if (tab.value) shell.setTabTitle(tab.value.id, value)
  },
  registerCloseGuard: (guard) => {
    const registeredFor = tab.value?.id
    if (registeredFor) shell.setTabCloseGuard(registeredFor, guard)
    return () => {
      if (registeredFor) shell.setTabCloseGuard(registeredFor, null)
    }
  },
  // One tab per window in this phase — closing the tab closes the window
  // (tabs.ts, T033, replaces this with the neighbor-activation rule).
  closeSelf: () => shell.closeWindow(props.window.id),
})
</script>

<template>
  <div
    class="absolute flex flex-col overflow-hidden rounded-lg border bg-background shadow-lg"
    :class="active ? 'border-foreground/40' : 'border-border'"
    :style="{
      left: `${window.x}px`,
      top: `${window.y}px`,
      width: `${window.width}px`,
      height: `${window.height}px`,
      zIndex: window.stack,
    }"
    role="group"
    :aria-label="title"
    @pointerdown="shell.focusWindow(window.id)"
  >
    <div
      class="flex shrink-0 items-center gap-2 border-b border-border bg-muted/40 px-3 py-2"
    >
      <Icon
        v-if="app"
        :name="app.icon"
        class="h-3.5 w-3.5 shrink-0"
        :aria-hidden="true"
      />
      <span class="min-w-0 flex-1 truncate text-sm font-medium">{{
        title
      }}</span>
      <button
        type="button"
        class="shrink-0 rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        :aria-label="t('shell.window.close')"
        @click.stop="shell.closeWindow(window.id)"
      >
        <Icon name="lucide:x" class="h-3.5 w-3.5" :aria-hidden="true" />
      </button>
    </div>
    <div class="min-h-0 flex-1 overflow-hidden">
      <component :is="component" v-if="component" />
    </div>
  </div>
</template>
