// Password manager search (spec 034-password-manager, FR-007, research R11): filters the headers
// the window already holds. Exactly four things are searched — title, username, URL and the tag
// names — never a note, a password or any other field, so the search can never become a way to
// probe a secret. Pure, so `scripts/check-passwords-search.ts` runs it without vue.

/** What the search looks at; an `ItemHeader` satisfies it, and so does nothing that carries more. */
export type SearchableHeader = {
  id: string
  title: string | null
  username: string | null
  url: string | null
  tags: { id: string; name: string }[]
}

/** Case- and accent-insensitive, and a decomposed umlaut equals a composed one: "bank" finds
 * "Bänk". */
export function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
}

/** The words of a query, folded; empty for a blank query. */
function terms(query: string): string[] {
  return fold(query)
    .split(/\s+/)
    .filter((word) => word.length > 0)
}

/** A reference placeholder (spec 036, `contracts/references.md`). The grammar itself lives in Rust
 * (`passwords/references.rs`); this only keeps the search from matching the raw token, so a search
 * for "password" or a piece of an id does not find every entry with a reference. */
const PLACEHOLDER =
  /\{\$[0-9A-Fa-f]{8}-(?:[0-9A-Fa-f]{4}-){3}[0-9A-Fa-f]{12}:(?:username|password|extra:(?:\\[\s\S]|[^}\\])+)\}/g

/** A text cut into plain parts and placeholders, for a list that shows a mark instead of the raw
 * token (spec 036, FR-047). Who the mark points at is left to the entry view, which asks Rust. */
export function placeholderParts(
  text: string,
): { kind: 'text' | 'mark'; text: string }[] {
  const parts: { kind: 'text' | 'mark'; text: string }[] = []
  let last = 0
  for (const match of text.matchAll(PLACEHOLDER)) {
    const start = match.index ?? 0
    if (start > last)
      parts.push({ kind: 'text', text: text.slice(last, start) })
    parts.push({ kind: 'mark', text: match[0] })
    last = start + match[0].length
  }
  if (last < text.length) parts.push({ kind: 'text', text: text.slice(last) })
  return parts
}

function withoutPlaceholders(text: string | null): string | null {
  return text === null ? null : text.replace(PLACEHOLDER, ' ')
}

/** The folded texts a header is searched in. */
function haystack(header: SearchableHeader): string[] {
  return [
    header.title,
    withoutPlaceholders(header.username),
    withoutPlaceholders(header.url),
    ...header.tags.map((tag) => tag.name),
  ]
    .filter(
      (text): text is string => typeof text === 'string' && text.trim() !== '',
    )
    .map(fold)
}

/** True when every word of the query is part of one of the searched texts. A blank query matches
 * everything. */
export function matchesQuery(header: SearchableHeader, query: string): boolean {
  const words = terms(query)
  if (words.length === 0) return true
  const texts = haystack(header)
  return words.every((word) => texts.some((text) => text.includes(word)))
}

/** The headers that match the query and, when given, carry the tag (`tagIds`: any of a group of
 * tags that show as one). */
export function filterHeaders<T extends SearchableHeader>(
  headers: readonly T[],
  options: { query: string; tagId?: string; tagIds?: readonly string[] },
): T[] {
  const words = terms(options.query)
  const wanted = options.tagIds ?? (options.tagId ? [options.tagId] : [])
  return headers.filter((header) => {
    if (
      wanted.length > 0 &&
      !header.tags.some((tag) => wanted.includes(tag.id))
    ) {
      return false
    }
    if (words.length === 0) return true
    const texts = haystack(header)
    return words.every((word) => texts.some((text) => text.includes(word)))
  })
}
