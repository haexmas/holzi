// Part of `pnpm check:passwords` (spec 036, FR-041, research R14): the cache of the attachment
// thumbnails (src/lib/passwords/thumbnails.ts). Rendering is injected, so the queue and the LRU are
// checked without a DOM.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  createThumbnailCache,
  THUMBNAIL_CACHE_SIZE,
  ThumbnailNotNow,
} from '../src/lib/passwords/thumbnails.ts'

/** A render whose calls stay open until the test settles them. */
function controlledRender() {
  const pending = new Map<
    string,
    { resolve: (url: string) => void; reject: (cause: unknown) => void }
  >()
  const calls: string[] = []
  const render = (key: string) =>
    new Promise<string>((resolve, reject) => {
      calls.push(key)
      pending.set(key, { resolve, reject })
    })
  return { render, pending, calls }
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

test('the cache holds 200 thumbnails by default', () => {
  assert.equal(THUMBNAIL_CACHE_SIZE, 200)
})

test('the least recently used thumbnail goes first and its URL is revoked', async () => {
  const revoked: string[] = []
  const cache = createThumbnailCache({
    capacity: 2,
    render: async (key) => `blob:${key}`,
    revoke: (url) => revoked.push(url),
  })
  await cache.request('a')
  await cache.request('b')
  // Reading `a` makes `b` the least recently used.
  assert.deepEqual(cache.get('a'), { kind: 'ready', url: 'blob:a' })
  await cache.request('c')
  assert.equal(cache.get('b'), undefined)
  assert.deepEqual(revoked, ['blob:b'])
  assert.ok(cache.get('a'))
  assert.ok(cache.get('c'))
})

test('at most two renders run at once; a third waits for a free slot', async () => {
  const { render, pending, calls } = controlledRender()
  const cache = createThumbnailCache({ render, revoke: () => {} })
  const all = ['a', 'b', 'c'].map((key) => cache.request(key))
  await tick()
  assert.deepEqual(calls, ['a', 'b'])
  assert.equal(cache.inFlight(), 2)
  pending.get('a')?.resolve('blob:a')
  await tick()
  assert.deepEqual(calls, ['a', 'b', 'c'])
  pending.get('b')?.resolve('blob:b')
  pending.get('c')?.resolve('blob:c')
  const results = await Promise.all(all)
  assert.deepEqual(
    results.map((thumb) => thumb.kind),
    ['ready', 'ready', 'ready'],
  )
  assert.equal(cache.inFlight(), 0)
})

test('the same key asked twice renders once', async () => {
  const { render, pending, calls } = controlledRender()
  const cache = createThumbnailCache({ render, revoke: () => {} })
  const first = cache.request('a')
  const second = cache.request('a')
  await tick()
  pending.get('a')?.resolve('blob:a')
  assert.deepEqual(await first, await second)
  assert.deepEqual(calls, ['a'])
})

test('a failed render is remembered and not tried again', async () => {
  let tries = 0
  const cache = createThumbnailCache({
    render: async () => {
      tries += 1
      throw new Error('not an image')
    },
    revoke: () => {},
  })
  assert.deepEqual(await cache.request('broken'), { kind: 'failed' })
  assert.deepEqual(await cache.request('broken'), { kind: 'failed' })
  assert.equal(tries, 1)
})

test('clear revokes every URL and empties the cache', async () => {
  const revoked: string[] = []
  const cache = createThumbnailCache({
    render: async (key) => `blob:${key}`,
    revoke: (url) => revoked.push(url),
  })
  await cache.request('a')
  await cache.request('b')
  cache.clear()
  assert.deepEqual(revoked.sort(), ['blob:a', 'blob:b'])
  assert.equal(cache.get('a'), undefined)
})

test('a render that ends after clear revokes its URL instead of storing it', async () => {
  const { render, pending } = controlledRender()
  const revoked: string[] = []
  const cache = createThumbnailCache({
    render,
    revoke: (url) => revoked.push(url),
  })
  const late = cache.request('a')
  await tick()
  cache.clear()
  pending.get('a')?.resolve('blob:a')
  assert.deepEqual(await late, { kind: 'failed' })
  assert.deepEqual(revoked, ['blob:a'])
  assert.equal(cache.get('a'), undefined)
})

test('a render that fails for now is not remembered and is tried again', async () => {
  let calls = 0
  const cache = createThumbnailCache({
    render: async (key) => {
      calls += 1
      if (calls === 1) throw new ThumbnailNotNow('reading failed')
      return `blob:${key}`
    },
    revoke: () => {},
  })
  assert.deepEqual(await cache.request('a'), { kind: 'failed' })
  assert.equal(cache.get('a'), undefined)
  assert.deepEqual(await cache.request('a'), { kind: 'ready', url: 'blob:a' })
  assert.equal(calls, 2)
})
