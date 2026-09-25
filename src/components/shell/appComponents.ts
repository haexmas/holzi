import { defineAsyncComponent, type Component } from 'vue'

/**
 * Maps an app id (`lib/shell/apps.ts`) to the component it renders in a
 * tab (spec 015-workspace-shell, T017). Kept separate from `apps.ts` so
 * that pure module stays free of Vue and loadable by
 * `scripts/check-shell-state.ts` (plan research R9). Each entry is built
 * once at module load — not per call — so Vue's own async-component
 * loading cache is keyed by app id rather than reset on every render.
 */
const APP_COMPONENTS: Record<string, Component> = {
  'system.chat': defineAsyncComponent(
    () => import('~/components/apps/ChatApp.vue'),
  ),
  'system.settings': defineAsyncComponent(
    () => import('~/components/apps/SettingsApp.vue'),
  ),
  'system.federation': defineAsyncComponent(
    () => import('~/components/apps/FederationApp.vue'),
  ),
}

/** `undefined` for an `appId` no component is registered for — the caller (`ShellWindow.vue`)
 * treats that the same as an app the registry itself does not know (research R9): the tab cannot
 * render, so it is dropped rather than shown broken. */
export function getAppComponent(appId: string): Component | undefined {
  return APP_COMPONENTS[appId]
}
