<script setup lang="ts">
/**
 * One tab's content: its own `useShellTab()` context (spec
 * 015-workspace-shell, T034) — a *different* component instance per tab, so
 * Vue's provide/inject correctly scopes `tabId`/attention/guard to just
 * this tab, even while other tabs in the same window stay mounted
 * alongside it (research R8). Hidden via `v-show`, not `v-if`, when
 * inactive; lazy-mounted the first time it becomes active
 * (`shell.markTabMounted`), then left mounted. Spec 020: the content is
 * the tab's root `ShellRouterView`, which renders the app for the tab's
 * current location.
 */
import { computed, watchEffect } from 'vue'
import { getAppRoutes } from '~/components/shell/appRoutes'
import { provideShellTab } from '~/composables/useShellTab'
import type { ShellTab } from '~/lib/shell/types'

const props = defineProps<{
  tab: ShellTab
  windowId: string
  active: boolean
}>()

const shell = useShellStore()

const hasRoutes = computed(() => getAppRoutes(props.tab.appId) !== undefined)
const mounted = computed(() => shell.runtimeFor(props.tab.id).mounted)

provideShellTab({
  tabId: props.tab.id,
  windowId: props.windowId,
  appId: props.tab.appId,
  requestAttention: () => shell.setTabAttention(props.tab.id, true),
  clearAttention: () => shell.setTabAttention(props.tab.id, false),
  setTitle: (value) => shell.setTabTitle(props.tab.id, value),
  registerCloseGuard: (guard) => {
    shell.setTabCloseGuard(props.tab.id, guard)
    return () => shell.setTabCloseGuard(props.tab.id, null)
  },
  closeSelf: () => shell.closeTab(props.windowId, props.tab.id),
  openApp: (appId, at) => {
    shell.openApp(appId, at ?? null)
  },
  registerActionHandler: (actionId, handler) =>
    shell.registerTabActionHandler(props.tab.id, actionId, handler),
})

watchEffect(() => {
  if (props.active) shell.markTabMounted(props.tab.id)
})
</script>

<template>
  <div
    v-show="active"
    role="tabpanel"
    class="h-full min-h-0"
    :aria-hidden="!active"
  >
    <ShellRouterView v-if="hasRoutes && mounted" />
  </div>
</template>
