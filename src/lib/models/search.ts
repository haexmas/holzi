// Fuzzy model search for the chat composer's model picker (spec 031-chat-model-search, FR-002/003/004,
// research R1). Pure — no Vue import, so `scripts/check-chat-model-search.ts` runs it without a browser.
import Fuse from 'fuse.js'

export type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string }[]
}

type Entry = {
  providerId: string
  providerName: string
  modelIndex: number
  modelName: string
}

const entryKey = (providerId: string, modelIndex: number) =>
  `${providerId}#${modelIndex}`

/** Filters `groups` to those matching `query` against provider or model name (FR-002); an empty query
 * returns `groups` unchanged. Matching is fuzzy (FR-003): the query need not be contiguous, and a single
 * typo is tolerated. A group with no matching models is dropped entirely; surviving models keep their
 * original relative order (FR-004 — matches are not re-ranked across providers). */
export function filterModelGroups(
  groups: readonly ModelGroup[],
  query: string,
): ModelGroup[] {
  const trimmed = query.trim()
  if (trimmed === '') return [...groups]

  const entries: Entry[] = groups.flatMap((group) =>
    group.models.map((model, modelIndex) => ({
      providerId: group.providerId,
      providerName: group.providerName,
      modelIndex,
      modelName: model.name,
    })),
  )
  const fuse = new Fuse(entries, {
    threshold: 0.4,
    keys: ['providerName', 'modelName'],
  })
  const matched = new Set(
    fuse
      .search(trimmed)
      .map((r) => entryKey(r.item.providerId, r.item.modelIndex)),
  )

  return groups
    .map((group) => ({
      ...group,
      models: group.models.filter((_, modelIndex) =>
        matched.has(entryKey(group.providerId, modelIndex)),
      ),
    }))
    .filter((group) => group.models.length > 0)
}
