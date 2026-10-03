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
    }))
}

/** holzi's own apps followed by the extension apps. */
export function allApps(
  extensionAppList: readonly AppDefinition[],
): AppDefinition[] {
  return [...WM_APPS, ...extensionAppList]
}
