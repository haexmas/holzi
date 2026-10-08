/**
 * Thumbnails of the file browser (spec 044 FR-005, research R7): made and cached on disk in Rust,
 * held here as object URLs in the shared LRU (`lib/images/thumbnailCache.ts`, two at a time).
 * The key covers path, size and time, so a changed file gets a new thumbnail. Cleared when the last
 * file browser frame closes.
 */
import type { Entry } from '@bindings/Entry'
import type { SourceRef } from '@bindings/SourceRef'
import {
  createThumbnailCache,
  ThumbnailNotNow,
  type Thumbnail,
  type ThumbnailCache,
} from '~/lib/images/thumbnailCache'

/** What a render needs for a key: the latest entry seen with it. */
const targets = new Map<string, { source: SourceRef; entry: Entry }>()
let cache: ThumbnailCache | null = null

function keyOf(source: SourceRef, entry: Entry): string {
  const where =
    source.kind === 'storage' ? `storage:${source.storageId}` : 'device'
  return `${where}|${entry.path}|${entry.size ?? 0}|${entry.modifiedMs ?? 0}`
}

export function useFilesThumbnails() {
  const { thumbnailAsync } = useFiles()

  function theCache() {
    cache ??= createThumbnailCache({
      render: async (key) => {
        // Without a target the next request tries again; only an image Rust cannot decode is
        // remembered as a failure.
        const target = targets.get(key)
        if (!target) throw new ThumbnailNotNow('unknown thumbnail')
        const bytes = await thumbnailAsync(target.source, target.entry)
        return URL.createObjectURL(new Blob([bytes], { type: 'image/jpeg' }))
      },
      revoke: (url) => URL.revokeObjectURL(url),
    })
    return cache
  }

  /** The thumbnail of an image entry; renders it when it is not cached yet. */
  async function thumbnailFor(
    source: SourceRef,
    entry: Entry,
  ): Promise<Thumbnail> {
    const key = keyOf(source, entry)
    const stored = theCache().get(key)
    if (stored) return stored
    targets.set(key, { source, entry })
    return await theCache().request(key)
  }

  return { thumbnailFor }
}

/** Forgets every thumbnail and revokes its URL (the last frame closed). */
export function clearFilesThumbnails() {
  cache?.clear()
  cache = null
  targets.clear()
}
