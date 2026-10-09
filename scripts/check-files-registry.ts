// Part of `pnpm check:files` (spec 044, T019, T034): where a file browser tab stands survives as
// its tab location (session restore, spec 022).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  filesLocation,
  parseFilesPlace,
  type FilesPlace,
} from '../src/lib/files/registry.ts'

test('a device place round-trips through its tab location', () => {
  const place: FilesPlace = {
    source: { kind: 'device' },
    path: '/home/anna/Bilder',
    open: 'urlaub.jpg',
  }
  const location = filesLocation(place)
  assert.deepEqual(location, {
    path: '/device',
    query: { p: '/home/anna/Bilder', open: 'urlaub.jpg' },
  })
  assert.deepEqual(parseFilesPlace(location), place)
})

test('a storage place round-trips', () => {
  const place: FilesPlace = {
    source: { kind: 'storage', storageId: 'abc-123' },
    path: 'fotos/2026/',
  }
  assert.deepEqual(parseFilesPlace(filesLocation(place)), place)
})

test('the start is the device without a path', () => {
  assert.deepEqual(parseFilesPlace({ path: '/', query: {} }), {
    source: { kind: 'device' },
    path: null,
  })
})

test('unknown query keys and an open name with a slash are dropped', () => {
  assert.deepEqual(
    parseFilesPlace({
      path: '/device',
      query: { p: '/tmp', open: '../etc/passwd', extra: 'x' },
    }),
    { source: { kind: 'device' }, path: '/tmp' },
  )
})

test('an unknown location or a strange storage id is not a place', () => {
  assert.equal(parseFilesPlace({ path: '/elsewhere', query: {} }), undefined)
  assert.equal(parseFilesPlace({ path: '/storage/a b', query: {} }), undefined)
})

test('a search and its filter round-trip with the place', () => {
  const place = {
    source: { kind: 'device' as const },
    path: '/home/anna',
    q: 'urlaub',
    t: 'image,video',
    s: 'large',
    d: 'year',
  }
  assert.deepEqual(parseFilesPlace(filesLocation(place)), place)
})
