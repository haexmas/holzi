// What the window does when the data changes under it (spec 034-password-manager, US8): whether an
// open entry is still the one that was loaded, and how tags that two devices made twice show as one
// (until the cleanup at the next open merges them for good). Pure; the window holds the state.
import { fold } from './search.ts'

export type EntryFreshness = 'fresh' | 'changed' | 'deleted'

/** Compares the entry the view loaded with what the overview holds now. `present` is whether the
 * overview still lists the entry; `loadedToken` and `currentToken` are their `updatedAt`. Before the
 * overview has loaded once nothing is known, so the entry counts as fresh. */
export function entryFreshness(options: {
  overviewLoaded: boolean
  present: boolean
  loadedToken: string | null | undefined
  currentToken: string | null | undefined
}): EntryFreshness {
  if (!options.overviewLoaded) return 'fresh'
  if (!options.present) return 'deleted'
  if (
    options.loadedToken &&
    options.currentToken &&
    options.loadedToken !== options.currentToken
  ) {
    return 'changed'
  }
  return 'fresh'
}

export type TagLike = {
  id: string
  name: string
  color: string | null
  itemCount: number
}

export type MergedTag = {
  /** The smallest id of the group; what a filter link carries. */
  id: string
  /** Every id of the group, the smallest first. */
  ids: string[]
  name: string
  color: string | null
  itemCount: number
}

/** Tags whose names are equal by folding show as one: the smallest id leads, the count is the
 * number of entries that carry any of them. The order of first appearance stays. */
export function mergeTagsForDisplay(
  tags: readonly TagLike[],
  headers: readonly { id: string; tags: readonly { id: string }[] }[],
): MergedTag[] {
  const groups = new Map<string, TagLike[]>()
  for (const tag of tags) {
    const key = fold(tag.name)
    const group = groups.get(key)
    if (group) group.push(tag)
    else groups.set(key, [tag])
  }
  return [...groups.values()].map((group) => {
    const sorted = [...group].sort((a, b) => a.id.localeCompare(b.id))
    const primary = sorted[0]!
    const ids = sorted.map((tag) => tag.id)
    if (group.length === 1) {
      return {
        id: primary.id,
        ids,
        name: primary.name,
        color: primary.color,
        itemCount: primary.itemCount,
      }
    }
    const wanted = new Set(ids)
    const itemCount = headers.filter((header) =>
      header.tags.some((tag) => wanted.has(tag.id)),
    ).length
    return {
      id: primary.id,
      ids,
      name: primary.name,
      color: sorted.find((tag) => tag.color)?.color ?? null,
      itemCount,
    }
  })
}

/** The ids a filter by `tagId` has to accept: the whole merged group, or just the id. */
export function tagIdsFor(
  tagId: string,
  merged: readonly MergedTag[],
): string[] {
  return merged.find((tag) => tag.ids.includes(tagId))?.ids ?? [tagId]
}
