// Per-tab location and back/forward history for the Workspace-Shell (spec 020-tab-navigation,
// T009, data-model.md). Pure data and pure functions — every reducer returns a new history (or the
// very same object for a no-op) and never mutates its input. No Nuxt auto-imports, relative
// sibling imports only (015 plan research R6).

/** At most this many entries per tab (FR-009); the oldest entry drops first. */
export const MAX_HISTORY_ENTRIES = 50

/** The history list (long press / right click on back or forward) shows at most this many
 * entries per direction (FR-016). */
export const HISTORY_LIST_LIMIT = 15

/** Where a tab stands inside its app (FR-001): an app-relative path starting with `/` plus the
 * view's presentation state (filter, search, sort) as query. Strings only, so it survives a JSON
 * round trip and can move with its tab to another window. */
export type TabLocation = {
  path: string
  query: Record<string, string>
}

/** `title` is the title the tab showed when this entry was left (research R9); `null` while the
 * entry is current or was never left. */
export type HistoryEntry = {
  location: TabLocation
  title: string | null
}

/** One tab's linear history: `1 ≤ entries.length ≤ MAX_HISTORY_ENTRIES`,
 * `0 ≤ index < entries.length`. */
export type TabHistory = {
  entries: HistoryEntry[]
  index: number
}

/** Leading `/`, no empty segments, no trailing `/` except for the start location. */
export function normalizePath(path: string): string {
  const segments = path.split('/').filter((segment) => segment.length > 0)
  return `/${segments.join('/')}`
}

/** Accepts `'/models/hf/x?sort=size'` or an already structured location. */
export function parseLocation(input: string | TabLocation): TabLocation {
  if (typeof input !== 'string') {
    return { path: normalizePath(input.path), query: { ...input.query } }
  }
  const queryStart = input.indexOf('?')
  const rawPath = queryStart === -1 ? input : input.slice(0, queryStart)
  const query: Record<string, string> = {}
  if (queryStart !== -1) {
    for (const [key, value] of new URLSearchParams(input.slice(queryStart + 1)))
      query[key] = value
  }
  return { path: normalizePath(rawPath), query }
}

/** The inverse of `parseLocation`, with query keys sorted for a stable string. */
export function formatLocation(location: TabLocation): string {
  const keys = Object.keys(location.query).sort()
  if (keys.length === 0) return location.path
  const params = new URLSearchParams()
  for (const key of keys) params.set(key, location.query[key] ?? '')
  return `${location.path}?${params.toString()}`
}

/** Same normalized path and the same query pairs, regardless of key order (FR-006). */
export function locationsEqual(a: TabLocation, b: TabLocation): boolean {
  return formatLocation(parseLocation(a)) === formatLocation(parseLocation(b))
}

export function createHistory(start: string | TabLocation = '/'): TabHistory {
  return {
    entries: [{ location: parseLocation(start), title: null }],
    index: 0,
  }
}

/** The current entry's location. The invariant guarantees an entry at `index`. */
export function currentLocation(history: TabHistory): TabLocation {
  const entry = history.entries[history.index]
  if (!entry) throw new Error('tab history index out of range')
  return entry.location
}

export function canGoBack(history: TabHistory): boolean {
  return history.index > 0
}

export function canGoForward(history: TabHistory): boolean {
  return history.index < history.entries.length - 1
}

/** A new view (FR-004): drops every forward entry, remembers `leavingTitle` on the entry being
 * left, appends `to`, and trims to `MAX_HISTORY_ENTRIES`. Equal to the current location → no-op
 * (FR-006). */
export function push(
  history: TabHistory,
  to: string | TabLocation,
  leavingTitle: string | null = null,
): TabHistory {
  const location = parseLocation(to)
  if (locationsEqual(currentLocation(history), location)) return history
  const kept = history.entries.slice(0, history.index + 1)
  const entries = [
    ...kept.slice(0, -1),
    ...kept.slice(-1).map((entry) => ({ ...entry, title: leavingTitle })),
    { location, title: null },
  ]
  const overflow = Math.max(0, entries.length - MAX_HISTORY_ENTRIES)
  return {
    entries: entries.slice(overflow),
    index: entries.length - 1 - overflow,
  }
}

/** The same view shown differently (FR-005): only the current entry changes. */
export function replace(
  history: TabHistory,
  to: string | TabLocation,
): TabHistory {
  const location = parseLocation(to)
  return {
    entries: history.entries.map((entry, i) =>
      i === history.index ? { location, title: null } : entry,
    ),
    index: history.index,
  }
}

/** Back (`delta < 0`) or forward (`delta > 0`); outside the bounds or `delta = 0` → no-op
 * (FR-007). The entry being left keeps `leavingTitle`, the target's title resets to `null`. */
export function go(
  history: TabHistory,
  delta: number,
  leavingTitle: string | null = null,
): TabHistory {
  const target = history.index + delta
  if (delta === 0 || target < 0 || target >= history.entries.length)
    return history
  return {
    entries: history.entries.map((entry, i) => {
      if (i === history.index) return { ...entry, title: leavingTitle }
      if (i === target) return { ...entry, title: null }
      return entry
    }),
    index: target,
  }
}

/** Removes a stale entry (research R10, FR-014). Removing the current entry moves to its neighbor
 * in travel direction: `-1` = the older one, `1` = the newer one (each clamped). A history is never
 * emptied. */
export function removeEntry(
  history: TabHistory,
  at: number,
  direction: -1 | 1 = 1,
): TabHistory {
  if (history.entries.length <= 1 || at < 0 || at >= history.entries.length)
    return history
  const entries = history.entries.filter((_, i) => i !== at)
  let index = history.index
  if (at < history.index) index -= 1
  else if (at === history.index)
    index =
      direction < 0
        ? Math.max(0, index - 1)
        : Math.min(index, entries.length - 1)
  return { entries, index }
}

export type HistoryListItem = { steps: number; entry: HistoryEntry }

/** Older entries, nearest first, with the (negative) number of steps `go` needs to reach them. */
export function backList(history: TabHistory): HistoryListItem[] {
  const items: HistoryListItem[] = []
  for (
    let i = history.index - 1;
    i >= 0 && items.length < HISTORY_LIST_LIMIT;
    i--
  ) {
    const entry = history.entries[i]
    if (entry) items.push({ steps: i - history.index, entry })
  }
  return items
}

/** Newer entries, nearest first, with the (positive) number of steps `go` needs to reach them. */
export function forwardList(history: TabHistory): HistoryListItem[] {
  const items: HistoryListItem[] = []
  for (
    let i = history.index + 1;
    i < history.entries.length && items.length < HISTORY_LIST_LIMIT;
    i++
  ) {
    const entry = history.entries[i]
    if (entry) items.push({ steps: i - history.index, entry })
  }
  return items
}

/** The location with `patch` merged into its query; `null` removes a key (`setQuery`, FR-005). */
export function withQuery(
  location: TabLocation,
  patch: Record<string, string | null>,
): TabLocation {
  const merged: Record<string, string | null> = { ...location.query, ...patch }
  const query = Object.fromEntries(
    Object.entries(merged).filter(
      (entry): entry is [string, string] => entry[1] !== null,
    ),
  )
  return { path: location.path, query }
}

/** Whether a link to `target` marks the current path as active; with `prefix`, any location below
 * `target` counts too — how a sidebar highlights the category of the current view (US1 AS7). */
export function isLocationActive(
  currentPath: string,
  target: string,
  prefix = false,
): boolean {
  const targetPath = parseLocation(target).path
  const current = normalizePath(currentPath)
  if (current === targetPath) return true
  if (!prefix) return false
  return targetPath === '/' || current.startsWith(`${targetPath}/`)
}
