// Password manager registry (spec 034-password-manager, research R13): the places of the app as
// route patterns with fixed title keys. Pure — the Vue side attaches the components in
// `components/wm/appRoutes.ts`. No title, name or value of an entry ever goes into a path, a
// query or a tab title; places carry opaque ids only (FR-039, FR-040).
import type { TabLocation } from '../wm/navigation.ts'
import { matchRoute, type RoutePattern } from '../wm/routeMatch.ts'

export type PasswordsLocation = {
  id: 'list' | 'folder' | 'entry' | 'history' | 'trash' | 'generator' | 'import'
  /** Route pattern relative to `/` (`''` is `/` itself). */
  pattern: string
  /** Static title in the history list and the tab; never built from a stored value. */
  titleKey: string
}

/** Longer patterns first only matter for readability: a pattern never matches a longer path. */
export const PASSWORDS_LOCATIONS: readonly PasswordsLocation[] = [
  { id: 'list', pattern: '', titleKey: 'wm.passwords.list' },
  { id: 'folder', pattern: 'folder/:id', titleKey: 'wm.passwords.folder' },
  {
    id: 'history',
    pattern: 'entry/:id/history',
    titleKey: 'wm.passwords.history',
  },
  { id: 'entry', pattern: 'entry/:id', titleKey: 'wm.passwords.entry' },
  { id: 'trash', pattern: 'trash', titleKey: 'wm.passwords.trash' },
  { id: 'generator', pattern: 'generator', titleKey: 'wm.passwords.generator' },
  { id: 'import', pattern: 'import', titleKey: 'wm.passwords.import' },
]

/** A `RoutePattern` that also names its location, so `appRoutes.ts` can attach the view. */
export type PasswordsRoutePattern = RoutePattern & {
  locationId?: PasswordsLocation['id']
  children?: readonly PasswordsRoutePattern[]
}

/** A root `/` with one flat child per place, so the list is never rendered together with a
 * detail view. */
export function passwordsRoutePatterns(): PasswordsRoutePattern[] {
  return [
    {
      path: '/',
      children: PASSWORDS_LOCATIONS.map((location) => ({
        path: location.pattern,
        titleKey: location.titleKey,
        locationId: location.id,
      })),
    },
  ]
}

export function locationFor(
  path: string,
): { location: PasswordsLocation; params: Record<string, string> } | undefined {
  const match = matchRoute(passwordsRoutePatterns(), path)
  const id = match?.chain[1]?.locationId
  const location = PASSWORDS_LOCATIONS.find((candidate) => candidate.id === id)
  return location && match ? { location, params: match.params } : undefined
}

const OPAQUE_ID = /^[A-Za-z0-9-]{1,64}$/
/** Search text is the only free text a place may carry (title, username, URL and tag names are
 * all that the search looks at); it is never a secret and stays short. */
const MAX_QUERY_LENGTH = 200

/** True when a location can be stored in the tab history and the saved session without leaking a
 * value: it is a known place, its path params are opaque ids, and its query holds only the search
 * text `q`, a tag id `tag` and the edit flag `edit`. */
export function isSecretFreeLocation(location: TabLocation): boolean {
  const found = locationFor(location.path)
  if (!found) return false
  for (const value of Object.values(found.params)) {
    if (!OPAQUE_ID.test(value)) return false
  }
  for (const [key, value] of Object.entries(location.query)) {
    if (key === 'q') {
      if (value.length > MAX_QUERY_LENGTH) return false
    } else if (key === 'tag') {
      if (!OPAQUE_ID.test(value)) return false
    } else if (key === 'edit') {
      if (value !== '' && value !== '1') return false
    } else {
      return false
    }
  }
  return true
}
