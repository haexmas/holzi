// Filters by type, size and time (spec 044 FR-029, T057): the same rules for the open folder (here)
// and for search hits (Rust, `files/search.rs`). The categories mirror `category` in
// `src-tauri/src/files/kind.rs`; `scripts/check-files-filters.ts` runs the same cases as its tests.
// A tab keeps its filter in its location (`t`, `s`, `d`), so session restore brings it back.
import type { FileCategory } from '../../types/bindings/FileCategory.ts'
import type { SearchFilters } from '../../types/bindings/SearchFilters.ts'
import { extensionOf, viewerKind } from './viewerKind.ts'

export const CATEGORIES: readonly FileCategory[] = [
  'image',
  'video',
  'audio',
  'document',
  'text',
]

const DOCUMENTS = new Set([
  'pdf',
  'doc',
  'docx',
  'odt',
  'rtf',
  'xls',
  'xlsx',
  'ods',
  'ppt',
  'pptx',
  'odp',
  'epub',
])

/** The category of a file by its name; `null` for anything else (archives, programs, …). */
export function categoryOf(name: string): FileCategory | null {
  const ext = extensionOf(name)
  if (DOCUMENTS.has(ext)) return 'document'
  // A photo from a phone is an image, even where the web view cannot show it.
  if (ext === 'heic' || ext === 'heif') return 'image'
  const kind = viewerKind(name)
  return kind === 'image' ||
    kind === 'video' ||
    kind === 'audio' ||
    kind === 'text'
    ? kind
    : null
}

export const SIZE_RANGES = ['small', 'medium', 'large'] as const
export type SizeRange = (typeof SIZE_RANGES)[number]
export const DATE_RANGES = ['day', 'week', 'month', 'year'] as const
export type DateRange = (typeof DATE_RANGES)[number]

/** What a tab filters by. */
export type FilesFilter = {
  types: FileCategory[]
  size: SizeRange | null
  date: DateRange | null
}

export const NO_FILTER: FilesFilter = { types: [], size: null, date: null }

const MIB = 1024 * 1024
const DAY_MS = 24 * 60 * 60 * 1000

/** Bytes of each size range: small under 1 MiB, medium up to 100 MiB, large above. */
const SIZES: Record<SizeRange, { min?: number; max?: number }> = {
  small: { max: MIB - 1 },
  medium: { min: MIB, max: 100 * MIB },
  large: { min: 100 * MIB + 1 },
}

const DAYS: Record<DateRange, number> = {
  day: 1,
  week: 7,
  month: 30,
  year: 365,
}

export function isFiltering(filter: FilesFilter): boolean {
  return filter.types.length > 0 || filter.size !== null || filter.date !== null
}

/** The filter of a tab's location query. */
export function parseFilter(query: {
  t?: string
  s?: string
  d?: string
}): FilesFilter {
  const types = (query.t ?? '')
    .split(',')
    .filter((type): type is FileCategory =>
      (CATEGORIES as readonly string[]).includes(type),
    )
  const size = (SIZE_RANGES as readonly string[]).includes(query.s ?? '')
    ? (query.s as SizeRange)
    : null
  const date = (DATE_RANGES as readonly string[]).includes(query.d ?? '')
    ? (query.d as DateRange)
    : null
  return { types: [...new Set(types)], size, date }
}

/** The query keys of a filter; `null` removes a key. */
export function formatFilter(
  filter: FilesFilter,
): Record<'t' | 's' | 'd', string | null> {
  return {
    t: filter.types.length ? filter.types.join(',') : null,
    s: filter.size,
    d: filter.date,
  }
}

/** The filter as Rust's search takes it, with the time range counted back from `now`. */
export function searchFilters(filter: FilesFilter, now: number): SearchFilters {
  const out: SearchFilters = {}
  if (filter.types.length) out.types = filter.types
  if (filter.size) {
    const { min, max } = SIZES[filter.size]
    if (min !== undefined) out.sizeMin = min
    if (max !== undefined) out.sizeMax = max
  }
  if (filter.date) out.modifiedFrom = now - DAYS[filter.date] * DAY_MS
  return out
}

/** The part of an entry a filter reads. */
export type FilterableEntry = {
  name: string
  kind: 'file' | 'dir'
  size: number | null
  modifiedMs: number | null
}

/** Whether `entry` passes `filter` (as `SearchFilters::admits` in Rust): type and size let only
 * files through. */
export function admits(
  entry: FilterableEntry,
  filter: FilesFilter,
  now: number,
): boolean {
  const rules = searchFilters(filter, now)
  const filesOnly =
    rules.types !== undefined ||
    rules.sizeMin !== undefined ||
    rules.sizeMax !== undefined
  if (entry.kind === 'dir' && filesOnly) return false
  if (rules.types) {
    const category = categoryOf(entry.name)
    if (!category || !rules.types.includes(category)) return false
  }
  const size = entry.size ?? 0
  if (rules.sizeMin !== undefined && size < rules.sizeMin) return false
  if (rules.sizeMax !== undefined && size > rules.sizeMax) return false
  if (rules.modifiedFrom !== undefined) {
    if (entry.modifiedMs === null || entry.modifiedMs < rules.modifiedFrom)
      return false
  }
  return true
}
