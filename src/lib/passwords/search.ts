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

/** The folded texts a header is searched in. */
function haystack(header: SearchableHeader): string[] {
  return [
    header.title,
    header.username,
    header.url,
    ...header.tags.map((tag) => tag.name),
  ]
    .filter((text): text is string => typeof text === 'string' && text !== '')
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

/** The headers that match the query and, when given, carry the tag. */
export function filterHeaders<T extends SearchableHeader>(
  headers: readonly T[],
  options: { query: string; tagId?: string },
): T[] {
  const words = terms(options.query)
  return headers.filter((header) => {
    if (options.tagId && !header.tags.some((tag) => tag.id === options.tagId)) {
      return false
    }
    if (words.length === 0) return true
    const texts = haystack(header)
    return words.every((word) => texts.some((text) => text.includes(word)))
  })
}
