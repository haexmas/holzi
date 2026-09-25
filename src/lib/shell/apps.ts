// App registry for the Workspace-Shell (spec 015-workspace-shell, T015). Pure data — the
// id-to-component mapping lives separately in src/components/shell/appComponents.ts (T017), so
// this module stays free of Vue/Nuxt and loadable by scripts/check-shell-state.ts.
import type { Size } from './types.ts'

/** Describes what runs in a tab. `id` is namespaced (`system.*` here; a future `extension.*` is
 * valid on the wire per research R9 but unknown to this registry — an unresolvable tab is dropped
 * on hydrate, FR-025). `defaultSize`/`minSize` apply to a *new window* opened with this app as its
 * first tab; a window with more than one tab keeps its own geometry. */
export type ShellAppDefinition = {
  id: string
  /** i18n key for the localized name shown in the Launcher and tab. */
  titleKey: string
  /** Iconify name (e.g. `lucide:message-square`). */
  icon: string
  defaultSize: Size
  minSize: Size
  /** `false` = singleton: re-opening activates the existing tab instead of creating a second one
   * (FR-016). All three shipped apps are singletons in this spec (FR-017). */
  multiInstance: boolean
}

export const SHELL_APPS: readonly ShellAppDefinition[] = [
  {
    id: 'system.chat',
    titleKey: 'shell.apps.chat',
    icon: 'lucide:message-square',
    defaultSize: { width: 960, height: 640 },
    minSize: { width: 420, height: 360 },
    multiInstance: false,
  },
  {
    id: 'system.settings',
    titleKey: 'shell.apps.settings',
    icon: 'lucide:settings-2',
    defaultSize: { width: 760, height: 560 },
    minSize: { width: 420, height: 360 },
    multiInstance: false,
  },
  {
    id: 'system.federation',
    titleKey: 'shell.apps.federation',
    icon: 'lucide:share-2',
    defaultSize: { width: 640, height: 480 },
    minSize: { width: 360, height: 320 },
    multiInstance: false,
  },
]

/** `undefined` for an unknown `appId` — every caller (Launcher, `+` menu, hydrate) must handle
 * that case rather than assume the registry is exhaustive (research R9). `apps` defaults to the
 * shipped registry; the reducers (layoutState.ts, tabs.ts) take it as a parameter instead of
 * importing `SHELL_APPS` directly so a test can substitute a `multiInstance: true` app (T052)
 * without touching the shipped registry. */
export function getAppDefinition(
  appId: string,
  apps: readonly ShellAppDefinition[] = SHELL_APPS,
): ShellAppDefinition | undefined {
  return apps.find((app) => app.id === appId)
}
