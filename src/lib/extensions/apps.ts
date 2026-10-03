// Installed extensions as apps of the window manager (spec 017, US1, T043). Pure: the extensions
// store feeds it, `scripts/check-extensions-apps.ts` tests it.
import { WM_APPS, type AppDefinition } from '../wm/apps.ts'
import type { ExtensionSummary } from '../../types/bindings/ExtensionSummary.ts'

const PREFIX = 'extension.'

/** The app id of an extension's tabs. */
export function extensionAppId(extensionId: string): string {
  return PREFIX + extensionId
}

/** The extension an app id names, or `null` for a holzi app. */
export function extensionIdOf(appId: string): string | null {
  return appId.startsWith(PREFIX) ? appId.slice(PREFIX.length) : null
}

/** States on this device in which an extension cannot open (data-model.md §Zustände). */
const UNAVAILABLE = new Set([
  'transferring',
  'signature_failed',
  'migration_failed',
])

/** The i18n key of a device state. */
export function statusKey(status: string): string {
  return `extensions.status.${status}`
}

/** The i18n key of the error kind of a device state, or `null` without one: a broken signature
 * names the rule the bundle broke, a failed migration what went wrong. */
export function statusErrorKey(
  status: string,
  error: string | null | undefined,
): string | null {
  if (!error) return null
  return status === 'signature_failed'
    ? `extensions.install.errors.${error}`
    : `extensions.statusError.${error}`
}

/** One app per installed and enabled extension, with its own name and icon. */
export function extensionApps(
  extensions: readonly ExtensionSummary[],
  icons: Readonly<Record<string, string>>,
): AppDefinition[] {
  return extensions
    .filter((e) => e.state === 'installed' && e.enabled)
    .map((e) => ({
      id: extensionAppId(e.id),
      titleKey: 'wm.apps.extension',
      title: e.title,
      icon: 'lucide:puzzle',
      ...(icons[e.id] ? { iconUrl: icons[e.id] } : {}),
      defaultSize: { width: 960, height: 640 },
      minSize: { width: 360, height: 320 },
      multiInstance: !e.singleInstance,
      tabTitle: 'app' as const,
      ...(e.statusHere && UNAVAILABLE.has(e.statusHere)
        ? { unavailableKey: statusKey(e.statusHere) }
        : {}),
    }))
}

/** holzi's own apps followed by the extension apps. */
export function allApps(
  extensionAppList: readonly AppDefinition[],
): AppDefinition[] {
  return [...WM_APPS, ...extensionAppList]
}
