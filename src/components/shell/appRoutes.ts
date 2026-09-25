import { defineAsyncComponent, type Component } from 'vue'
import type { RoutePattern } from '~/lib/shell/routeMatch'

/**
 * Per-app route tables for tab navigation (spec 020-tab-navigation, T020,
 * research R4, contracts/tab-navigation-contract.md §1) — replaces 015's
 * `appComponents.ts`. Kept separate from `lib/shell/apps.ts` so that pure
 * module stays free of Vue. Each component is built once at module load, so
 * Vue's async-component cache is keyed per app rather than reset per render.
 *
 * A record's `component` is optional below the root: an app can keep one
 * mounted root component and just read `useTabRouter().route` (the chat does,
 * so a conversation switch never remounts it and loses a running reply).
 */
export type AppRouteRecord = RoutePattern & {
  component?: Component
  children?: readonly AppRouteRecord[]
}

const ChatApp = defineAsyncComponent(
  () => import('~/components/apps/ChatApp.vue'),
)
const SettingsApp = defineAsyncComponent(
  () => import('~/components/apps/SettingsApp.vue'),
)
const FederationApp = defineAsyncComponent(
  () => import('~/components/apps/FederationApp.vue'),
)

const APP_ROUTES: Record<string, readonly AppRouteRecord[]> = {
  // Chat (research R10): one mounted root; children only carry the location.
  'system.chat': [
    {
      path: '/',
      component: ChatApp,
      children: [
        { path: '' },
        { path: 'thread/:id', titleKey: 'shell.chat.thread' },
      ],
    },
  ],
  // An app without its own routes has exactly the start location `/`.
  'system.settings': [{ path: '/', component: SettingsApp }],
  'system.federation': [{ path: '/', component: FederationApp }],
}

/** `undefined` for an `appId` no routes are registered for — `ShellTabPanel.vue` then renders
 * nothing, the same as an app the registry does not know (015 research R9). */
export function getAppRoutes(
  appId: string,
): readonly AppRouteRecord[] | undefined {
  return APP_ROUTES[appId]
}
