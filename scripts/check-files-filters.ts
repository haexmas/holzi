// Part of `pnpm check:files` (spec 044, T057): categories as in `src-tauri/src/files/kind_tests.rs`,
// the filter of a tab's location, and which entries a filter lets through.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  admits,
  categoryOf,
  type FilesFilter,
  formatFilter,
  isFiltering,
  NO_FILTER,
  parseFilter,
  searchFilters,
} from '../src/lib/files/filters.ts'

test('categories follow the kind with documents and phone photos, as in Rust', () => {
  for (const [name, category] of [
    ['notiz.txt', 'text'],
    ['daten.json', 'text'],
    ['brief.pdf', 'document'],
    ['bericht.docx', 'document'],
    ['tabelle.ODS', 'document'],
    ['vortrag.pptx', 'document'],
    ['foto.JPG', 'image'],
    ['handy.heic', 'image'],
    ['film.mkv', 'video'],
    ['lied.opus', 'audio'],
    ['archiv.zip', null],
    ['ohne-endung', null],
  ] as const) {
    assert.equal(categoryOf(name), category, name)
  }
})

test('a filter survives the location and drops what it does not know', () => {
  const filter: FilesFilter = {
    types: ['image', 'video'],
    size: 'large',
    date: 'week',
  }
  const query = formatFilter(filter)
  assert.deepEqual(query, { t: 'image,video', s: 'large', d: 'week' })
  assert.deepEqual(
    parseFilter({ t: query.t!, s: query.s!, d: query.d! }),
    filter,
  )
  assert.deepEqual(parseFilter({ t: 'image,nope,image', s: 'huge', d: '' }), {
    types: ['image'],
    size: null,
    date: null,
  })
  assert.deepEqual(formatFilter(NO_FILTER), { t: null, s: null, d: null })
  assert.equal(isFiltering(NO_FILTER), false)
  assert.equal(isFiltering({ ...NO_FILTER, date: 'day' }), true)
})

const NOW = Date.UTC(2026, 9, 9)
const DAY = 24 * 60 * 60 * 1000

test('the search gets numbers: sizes in bytes, the time counted back from now', () => {
  assert.deepEqual(searchFilters(NO_FILTER, NOW), {})
  assert.deepEqual(
    searchFilters({ types: ['text'], size: 'medium', date: 'month' }, NOW),
    {
      types: ['text'],
      sizeMin: 1024 * 1024,
      sizeMax: 100 * 1024 * 1024,
      modifiedFrom: NOW - 30 * DAY,
    },
  )
  assert.deepEqual(searchFilters({ ...NO_FILTER, size: 'small' }, NOW), {
    sizeMax: 1024 * 1024 - 1,
  })
})

const file = (name: string, size: number, ageDays: number) => ({
  name,
  kind: 'file' as const,
  size,
  modifiedMs: NOW - ageDays * DAY,
})
const dir = (name: string) => ({
  name,
  kind: 'dir' as const,
  size: null,
  modifiedMs: NOW,
})

test('type and size let only files through; the time applies to folders too', () => {
  const images: FilesFilter = { ...NO_FILTER, types: ['image'] }
  assert.ok(admits(file('a.jpg', 10, 0), images, NOW))
  assert.ok(!admits(file('a.txt', 10, 0), images, NOW))
  assert.ok(!admits(dir('Bilder'), images, NOW))
  const small: FilesFilter = { ...NO_FILTER, size: 'small' }
  assert.ok(admits(file('a.txt', 10, 0), small, NOW))
  assert.ok(!admits(file('a.txt', 2 * 1024 * 1024, 0), small, NOW))
  const week: FilesFilter = { ...NO_FILTER, date: 'week' }
  assert.ok(admits(file('a.txt', 10, 6), week, NOW))
  assert.ok(!admits(file('a.txt', 10, 8), week, NOW))
  assert.ok(admits(dir('Neu'), week, NOW))
  assert.ok(admits(dir('Bilder'), NO_FILTER, NOW))
})
