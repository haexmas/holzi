// The file browser's view of a folder (spec 044 FR-003, FR-004), pure so `check-files-state.ts`
// runs it without a DOM: sorting, hidden entries, the path bar and the parent folder.
import type { Entry } from '@bindings/Entry'

export const SORT_KEYS = ['name', 'size', 'modified', 'type'] as const
export type SortKey = (typeof SORT_KEYS)[number]
export type Sort = { key: SortKey; ascending: boolean }

export const DEFAULT_SORT: Sort = { key: 'name', ascending: true }

/** `name:asc` and friends, as stored in the device preference `files.sort`. */
export function parseSort(value: string | null | undefined): Sort {
  const [key, direction] = (value ?? '').split(':')
  if (!SORT_KEYS.includes(key as SortKey)) return DEFAULT_SORT
  if (direction !== 'asc' && direction !== 'desc') return DEFAULT_SORT
  return { key: key as SortKey, ascending: direction === 'asc' }
}

export function formatSort(sort: Sort): string {
  return `${sort.key}:${sort.ascending ? 'asc' : 'desc'}`
}

const collator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: 'base',
})

function typeOf(entry: Entry): string {
  if (entry.kind === 'dir') return ''
  const dot = entry.name.lastIndexOf('.')
  return dot > 0 ? entry.name.slice(dot + 1).toLowerCase() : ''
}

/** Folders first, then by `sort`; equal values fall back to the name. Hidden entries only when
 * `showHidden`. */
export function visibleEntries(
  entries: readonly Entry[],
  sort: Sort,
  showHidden: boolean,
): Entry[] {
  const direction = sort.ascending ? 1 : -1
  const byName = (a: Entry, b: Entry) => collator.compare(a.name, b.name)
  const byKey = (a: Entry, b: Entry): number => {
    switch (sort.key) {
      case 'size':
        return (a.size ?? 0) - (b.size ?? 0)
      case 'modified':
        return (a.modifiedMs ?? 0) - (b.modifiedMs ?? 0)
      case 'type':
        return collator.compare(typeOf(a), typeOf(b))
      default:
        return byName(a, b)
    }
  }
  return entries
    .filter((entry) => showHidden || !entry.hidden)
    .slice()
    .sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === 'dir' ? -1 : 1
      return direction * (byKey(a, b) || byName(a, b))
    })
}

/** The separator a path uses: `\` for a Windows drive path, else `/`. */
export function separatorOf(path: string): '/' | '\\' {
  return /^[A-Za-z]:\\/.test(path) ? '\\' : '/'
}

/** One part of the path bar. */
export type Crumb = { name: string; path: string }

/** The path bar of `path`: the root first, each folder down to `path` (FR-004). */
export function breadcrumbs(path: string): Crumb[] {
  const sep = separatorOf(path)
  if (sep === '\\') {
    const [drive = '', ...rest] = path.split('\\').filter(Boolean)
    const root = `${drive}\\`
    const crumbs: Crumb[] = [{ name: drive, path: root }]
    let current = root
    for (const part of rest) {
      current = current.endsWith('\\')
        ? `${current}${part}`
        : `${current}\\${part}`
      crumbs.push({ name: part, path: current })
    }
    return crumbs
  }
  const crumbs: Crumb[] = [{ name: '/', path: '/' }]
  let current = ''
  for (const part of path.split('/').filter(Boolean)) {
    current = `${current}/${part}`
    crumbs.push({ name: part, path: current })
  }
  return crumbs
}

/** The folder above `path`; `null` at a root. */
export function parentPath(path: string): string | null {
  const crumbs = breadcrumbs(path)
  return crumbs.length > 1 ? (crumbs.at(-2)?.path ?? null) : null
}

/** `path` joined with `name`, in the path's own style. */
export function childPath(path: string, name: string): string {
  const sep = separatorOf(path)
  return path.endsWith(sep) ? `${path}${name}` : `${path}${sep}${name}`
}
