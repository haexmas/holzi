/**
 * The cache of the attachment thumbnails (spec 036, FR-041, research R14): an LRU of rendered
 * thumbnails by key (the checksum of the bytes, so the same file in two entries renders once), with
 * at most two renders at a time, because each render briefly holds the full bytes of the file. A
 * failed render is remembered as a failure and not tried again, unless it threw `ThumbnailNotNow`
 * (the bytes could not be read this time). Rendering and revoking a URL are injected; this module
 * stays free of the DOM.
 */

export const THUMBNAIL_CACHE_SIZE = 200
export const THUMBNAIL_RENDERS_AT_ONCE = 2

export type Thumbnail = { kind: 'ready'; url: string } | { kind: 'failed' }

export interface ThumbnailCacheOptions {
  capacity?: number
  concurrency?: number
  /** Renders the thumbnail for a key and returns its object URL. */
  render: (key: string) => Promise<string>
  revoke: (url: string) => void
}

export interface ThumbnailCache {
  /** The stored thumbnail of a key; a read makes it the most recently used. */
  get: (key: string) => Thumbnail | undefined
  /** The thumbnail of a key, rendered when it is not stored yet. */
  request: (key: string) => Promise<Thumbnail>
  /** How many renders run right now. */
  inFlight: () => number
  /** Revokes every URL and forgets everything, also renders still running. */
  clear: () => void
}

const FAILED: Thumbnail = { kind: 'failed' }

/** A render that failed for now (the bytes could not be read); the next request tries again. */
export class ThumbnailNotNow extends Error {}

export function createThumbnailCache(
  options: ThumbnailCacheOptions,
): ThumbnailCache {
  const capacity = options.capacity ?? THUMBNAIL_CACHE_SIZE
  const concurrency = options.concurrency ?? THUMBNAIL_RENDERS_AT_ONCE
  // A Map keeps insertion order: the first key is the least recently used.
  const stored = new Map<string, Thumbnail>()
  const pending = new Map<string, Promise<Thumbnail>>()
  const waiting: (() => void)[] = []
  let running = 0
  let generation = 0

  function store(key: string, thumbnail: Thumbnail) {
    stored.delete(key)
    stored.set(key, thumbnail)
    while (stored.size > capacity) {
      const [oldest, evicted] = stored.entries().next().value as [
        string,
        Thumbnail,
      ]
      stored.delete(oldest)
      if (evicted.kind === 'ready') options.revoke(evicted.url)
    }
  }

  async function slot(): Promise<void> {
    if (running < concurrency) {
      running += 1
      return
    }
    // The slot is handed over by the render that ends, so `running` stays as it is.
    await new Promise<void>((resolve) => waiting.push(resolve))
  }

  function release() {
    const next = waiting.shift()
    if (next) next()
    else running -= 1
  }

  async function run(key: string, mine: number): Promise<Thumbnail> {
    await slot()
    try {
      const url = await options.render(key)
      if (mine !== generation) {
        options.revoke(url)
        return FAILED
      }
      const thumbnail: Thumbnail = { kind: 'ready', url }
      store(key, thumbnail)
      return thumbnail
    } catch (error) {
      if (mine === generation && !(error instanceof ThumbnailNotNow)) {
        store(key, FAILED)
      }
      return FAILED
    } finally {
      release()
    }
  }

  return {
    get(key) {
      const thumbnail = stored.get(key)
      if (thumbnail) store(key, thumbnail)
      return thumbnail
    },
    request(key) {
      const known = this.get(key)
      if (known) return Promise.resolve(known)
      const inProgress = pending.get(key)
      if (inProgress) return inProgress
      const mine = generation
      const promise = run(key, mine).finally(() => {
        if (pending.get(key) === promise) pending.delete(key)
      })
      pending.set(key, promise)
      return promise
    },
    inFlight: () => running,
    clear() {
      generation += 1
      for (const thumbnail of stored.values()) {
        if (thumbnail.kind === 'ready') options.revoke(thumbnail.url)
      }
      stored.clear()
      pending.clear()
    },
  }
}
