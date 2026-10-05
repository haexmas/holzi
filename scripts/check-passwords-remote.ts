// Part of `pnpm check:passwords` (spec 034-password-manager, US8): how the window treats changes
// from other devices (src/lib/passwords/remote.ts).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  entryFreshness,
  keepUnchanged,
  mergeTagsForDisplay,
  tagIdsFor,
} from '../src/lib/passwords/remote.ts'

const base = { overviewLoaded: true, present: true }

test('an entry that is unchanged, or not yet known to the overview, is fresh', () => {
  assert.equal(
    entryFreshness({ ...base, loadedToken: 'a', currentToken: 'a' }),
    'fresh',
  )
  assert.equal(
    entryFreshness({
      overviewLoaded: false,
      present: false,
      loadedToken: 'a',
      currentToken: undefined,
    }),
    'fresh',
  )
  assert.equal(
    entryFreshness({ ...base, loadedToken: null, currentToken: 'b' }),
    'fresh',
    'nothing was loaded yet, so nothing is stale',
  )
})

test('another token means the entry changed elsewhere, a missing one that it was deleted', () => {
  assert.equal(
    entryFreshness({ ...base, loadedToken: 'a', currentToken: 'b' }),
    'changed',
  )
  assert.equal(
    entryFreshness({
      ...base,
      present: false,
      loadedToken: 'a',
      currentToken: undefined,
    }),
    'deleted',
  )
})

const tag = (
  id: string,
  name: string,
  itemCount = 1,
  color: string | null = null,
) => ({
  id,
  name,
  color,
  itemCount,
})

test('tags with equal folded names show as one, the smallest id leads', () => {
  const merged = mergeTagsForDisplay(
    [
      tag('t-z', 'work', 2),
      tag('t-a', 'Work', 1, '#ef4444'),
      tag('t-m', 'Home'),
    ],
    [
      { id: 'i1', tags: [{ id: 't-a' }] },
      { id: 'i2', tags: [{ id: 't-z' }, { id: 't-a' }] },
      { id: 'i3', tags: [{ id: 't-z' }] },
      { id: 'i4', tags: [{ id: 't-m' }] },
    ],
  )
  assert.deepEqual(
    merged.map((t) => [t.id, t.ids, t.name, t.color, t.itemCount]),
    [
      ['t-a', ['t-a', 't-z'], 'Work', '#ef4444', 3],
      ['t-m', ['t-m'], 'Home', null, 1],
    ],
    'the count is entries that carry any of them, not the sum',
  )
})

test('umlaut spellings merge as the vault folds them, distinct tags stay apart', () => {
  const merged = mergeTagsForDisplay(
    [tag('a', 'Müller'), tag('b', 'Müller'), tag('c', 'Mueller')],
    [],
  )
  assert.equal(merged.length, 2)
})

test('a filter by any id of a merged tag accepts the whole group', () => {
  const merged = mergeTagsForDisplay(
    [tag('t-a', 'Work'), tag('t-z', 'work')],
    [],
  )
  assert.deepEqual(tagIdsFor('t-z', merged), ['t-a', 't-z'])
  assert.deepEqual(tagIdsFor('t-a', merged), ['t-a', 't-z'])
  assert.deepEqual(tagIdsFor('unknown', merged), ['unknown'])
})

test('a reload keeps the old object of every unchanged item', () => {
  const previous = [
    { id: '1', title: 'Mail', tags: ['a'] },
    { id: '2', title: 'Bank', tags: [] },
  ]
  const next = [
    { id: '1', title: 'Mail', tags: ['a'] },
    { id: '2', title: 'Bank 2', tags: [] },
    { id: '3', title: 'New', tags: [] },
  ]
  const kept = keepUnchanged(previous, next)
  assert.equal(kept[0], previous[0])
  assert.equal(kept[1], next[1])
  assert.equal(kept[2], next[2])
})

test('a reload that changed nothing returns the old list itself', () => {
  const previous = [{ id: '1', title: 'Mail' }]
  assert.equal(keepUnchanged(previous, [{ id: '1', title: 'Mail' }]), previous)
  assert.notEqual(keepUnchanged(previous, []), previous)
})
