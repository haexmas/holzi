// Settings registry (spec 023-settings-app, research R1–R3, data-model.md, contracts §1): the
// categories of the sidebar and every location of the settings app. One source for the route
// patterns, sidebar, overview rows, header and location titles. Pure — the Vue side attaches the
// components in `components/wm/appRoutes.ts`.
import { normalizePath, type TabHistory } from '../wm/navigation.ts'
import { matchRoute } from '../wm/routeMatch.ts'

export type SettingsCategoryId =
  'general' | 'appearance' | 'models' | 'agents' | 'federation'

export type SettingsCategory = {
  id: SettingsCategoryId
  /** The category's own location. */
  path: string
  icon: string
  titleKey: string
}

export type SettingsLocation = {
  id: string
  /** Route pattern relative to `/` (`''` is `/` itself). */
  pattern: string
  category: SettingsCategoryId
  /** Target of the header back arrow; only sub-views have one. */
  parent?: string
  /** Only locations shown as an overview row. */
  icon?: string
  /** Title in the header and the history list; may use the route params. */
  titleKey: string
  /** One line under the title where the location appears as a row (overview, search entry). The
   * header shows no description (operator decision 2026-09-26). */
  descriptionKey?: string
  overviewRow: boolean
  /** Synonyms the settings search matches besides title and description (FR-023); only locations
   * without route params are searchable. */
  keywordsKey?: string
  /** Labels of the single settings on this location's view, each its own search hit (FR-023). */
  settingKeys?: readonly string[]
}

function category(
  id: SettingsCategoryId,
  path: string,
  icon: string,
): SettingsCategory {
  return {
    id,
    path,
    icon,
    titleKey: `settings.categories.${id}.title`,
  }
}

/** In sidebar order (FR-005). */
export const SETTINGS_CATEGORIES: readonly SettingsCategory[] = [
  category('general', '/', 'lucide:sliders-horizontal'),
  category('appearance', '/appearance', 'lucide:palette'),
  category('models', '/models', 'lucide:box'),
  category('agents', '/agents', 'lucide:bot'),
  category('federation', '/federation', 'lucide:share-2'),
]

function categoryLocation(
  id: SettingsCategoryId,
  settingKeys?: readonly string[],
): SettingsLocation {
  return {
    id,
    pattern: SETTINGS_CATEGORIES.find((c) => c.id === id)!.path.slice(1),
    category: id,
    titleKey: `settings.categories.${id}.title`,
    overviewRow: false,
    keywordsKey: `settings.categories.${id}.keywords`,
    settingKeys,
  }
}

function subView(
  id: string,
  pattern: string,
  parent: string,
  options: { icon?: string; overviewRow?: boolean; row?: boolean } = {},
): SettingsLocation {
  const shownAsRow = (options.overviewRow ?? false) || (options.row ?? false)
  return {
    id,
    pattern,
    category: parent.split('.')[0] as SettingsCategoryId,
    parent,
    icon: options.icon,
    titleKey: `settings.locations.${id}.title`,
    descriptionKey: shownAsRow
      ? `settings.locations.${id}.description`
      : undefined,
    overviewRow: options.overviewRow ?? false,
    keywordsKey: pattern.includes(':')
      ? undefined
      : `settings.locations.${id}.keywords`,
  }
}

/** Overview rows appear in this order. */
export const SETTINGS_LOCATIONS: readonly SettingsLocation[] = [
  categoryLocation('general', [
    'settings.alias.label',
    'settings.sessionRestore.title',
  ]),
  categoryLocation('appearance', ['settings.colorScheme.label']),
  categoryLocation('models'),
  subView('models.default', 'models/default', 'models', {
    icon: 'lucide:star',
    overviewRow: true,
  }),
  subView('models.installed', 'models/installed', 'models', {
    icon: 'lucide:hard-drive',
    overviewRow: true,
  }),
  subView('models.download', 'models/download', 'models', {
    icon: 'lucide:download',
    overviewRow: true,
  }),
  subView(
    'models.download.search',
    'models/download/search',
    'models.download',
    { icon: 'lucide:search', row: true },
  ),
  subView(
    'models.download.repo',
    'models/download/repo/:owner/:name',
    'models.download.search',
  ),
  subView('models.speech', 'models/speech', 'models', {
    icon: 'lucide:mic',
    overviewRow: true,
  }),
  categoryLocation('agents'),
  subView('agents.providers', 'agents/providers', 'agents', {
    icon: 'lucide:plug',
    overviewRow: true,
  }),
  subView('agents.autonomy', 'agents/autonomy', 'agents', {
    icon: 'lucide:shield',
    overviewRow: true,
  }),
  subView('agents.denyRules', 'agents/deny-rules', 'agents', {
    icon: 'lucide:ban',
    overviewRow: true,
  }),
  categoryLocation('federation'),
  subView('federation.link', 'federation/devices/link', 'federation', {
    icon: 'lucide:qr-code',
    row: true,
  }),
]

/** A `RoutePattern` that also names its location, so `appRoutes.ts` can attach the view. */
export type SettingsRoutePattern = {
  path: string
  titleKey?: string
  locationId?: string
  children?: readonly SettingsRoutePattern[]
}

/** A root `/` with one flat child per location, so an overview is never rendered together with
 * one of its sub-views (research R1). */
export function settingsRoutePatterns(): SettingsRoutePattern[] {
  return [
    {
      path: '/',
      children: SETTINGS_LOCATIONS.map((location) => ({
        path: location.pattern,
        titleKey: location.titleKey,
        locationId: location.id,
      })),
    },
  ]
}

export function locationFor(
  path: string,
): { location: SettingsLocation; params: Record<string, string> } | undefined {
  const match = matchRoute(settingsRoutePatterns(), path)
  const id = match?.chain[1]?.locationId
  const location = SETTINGS_LOCATIONS.find((candidate) => candidate.id === id)
  return location && match ? { location, params: match.params } : undefined
}

export function categoryOf(path: string): SettingsCategoryId | undefined {
  return locationFor(path)?.location.category
}

export function overviewRows(
  categoryId: SettingsCategoryId,
): SettingsLocation[] {
  return SETTINGS_LOCATIONS.filter(
    (location) => location.category === categoryId && location.overviewRow,
  )
}

/** The path of a location without route params (every parent is one). */
export function locationPath(location: SettingsLocation): string {
  return normalizePath(`/${location.pattern}`)
}

/** The parent's path of a sub-view, `undefined` for a category. */
export function parentPathOf(location: SettingsLocation): string | undefined {
  const parent = SETTINGS_LOCATIONS.find(
    (candidate) => candidate.id === location.parent,
  )
  return parent ? locationPath(parent) : undefined
}

export type HeaderBack =
  { kind: 'back'; path: string } | { kind: 'push'; path: string }

/** FR-009: where the header arrow of the current location leads. Back to the previous station
 * when it lies in the same category (so Modelle → Installierte Modelle → Modelle herunterladen
 * returns to Installierte Modelle, and the search keeps its query), as a tab back without a
 * second entry; otherwise — after a deep link, a sidebar or search jump from another category —
 * to the registry parent. `undefined` for a category start page, which has no arrow. */
export function headerBack(history: TabHistory): HeaderBack | undefined {
  const current = history.entries[history.index]
  const here = current ? locationFor(current.location.path) : undefined
  const parentPath = here ? parentPathOf(here.location) : undefined
  if (!here || !parentPath) return undefined
  const previous = history.entries[history.index - 1]
  if (
    previous &&
    categoryOf(previous.location.path) === here.location.category
  ) {
    return { kind: 'back', path: normalizePath(previous.location.path) }
  }
  return { kind: 'push', path: normalizePath(parentPath) }
}
