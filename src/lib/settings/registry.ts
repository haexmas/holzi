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
  descriptionKey: string
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
  descriptionKey: string
  overviewRow: boolean
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
    descriptionKey: `settings.categories.${id}.description`,
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

function categoryLocation(id: SettingsCategoryId): SettingsLocation {
  return {
    id,
    pattern: SETTINGS_CATEGORIES.find((c) => c.id === id)!.path.slice(1),
    category: id,
    titleKey: `settings.categories.${id}.title`,
    descriptionKey: `settings.categories.${id}.description`,
    overviewRow: false,
  }
}

function subView(
  id: string,
  pattern: string,
  parent: string,
  options: { icon?: string; overviewRow?: boolean } = {},
): SettingsLocation {
  return {
    id,
    pattern,
    category: parent.split('.')[0] as SettingsCategoryId,
    parent,
    icon: options.icon,
    titleKey: `settings.locations.${id}.title`,
    descriptionKey: `settings.locations.${id}.description`,
    overviewRow: options.overviewRow ?? false,
  }
}

/** Overview rows appear in this order. */
export const SETTINGS_LOCATIONS: readonly SettingsLocation[] = [
  categoryLocation('general'),
  categoryLocation('appearance'),
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

export type HeaderBack = { kind: 'back' } | { kind: 'push'; path: string }

/** FR-009: the header arrow acts as tab back when the parent is the previous entry (keeping its
 * query, e.g. the search term), and navigates to the parent otherwise, e.g. after a deep link. */
export function headerBack(
  history: TabHistory,
  parentPath: string,
): HeaderBack {
  const previous = history.entries[history.index - 1]
  const target = normalizePath(parentPath)
  if (previous && normalizePath(previous.location.path) === target) {
    return { kind: 'back' }
  }
  return { kind: 'push', path: target }
}
