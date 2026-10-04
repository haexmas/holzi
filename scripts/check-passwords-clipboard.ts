// Part of `pnpm check:passwords` (spec 036, FR-013, FR-014, FR-021): the Ablage of entries and
// folders (src/lib/passwords/clipboard.ts). It holds ids and the mode only; nothing here touches
// the clipboard of the operating system.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  afterPaste,
  beginPaste,
  cutIds,
  fillAblage,
  settlePaste,
  splitMissing,
  toTargets,
  wouldCycle,
} from '../src/lib/passwords/clipboard.ts'

const GROUPS = [
  { id: 'work', name: 'Arbeit', parentId: null, sortOrder: 0 },
  { id: 'server', name: 'Server', parentId: 'work', sortOrder: 0 },
  { id: 'db', name: 'DB', parentId: 'server', sortOrder: 0 },
  { id: 'home', name: 'Privat', parentId: null, sortOrder: 0 },
]

test('ids become targets by whether they name a folder', () => {
  assert.deepEqual(toTargets(['a', 'work', 'b'], new Set(['work'])), [
    { kind: 'item', id: 'a' },
    { kind: 'group', id: 'work' },
    { kind: 'item', id: 'b' },
  ])
})

test('filling replaces what was there, drops doubles, and nothing is no Ablage', () => {
  const first = fillAblage([{ kind: 'item', id: 'a' }], 'cut')
  const second = fillAblage(
    [
      { kind: 'item', id: 'b' },
      { kind: 'item', id: 'b' },
    ],
    'copy',
  )
  assert.deepEqual(first, { targets: [{ kind: 'item', id: 'a' }], mode: 'cut' })
  assert.deepEqual(second, {
    targets: [{ kind: 'item', id: 'b' }],
    mode: 'copy',
  })
  assert.equal(fillAblage([], 'cut'), null)
})

test('a successful paste of a cut empties the Ablage, of a copy keeps it; a failure keeps both', () => {
  const cut = fillAblage([{ kind: 'item', id: 'a' }], 'cut')
  const copy = fillAblage([{ kind: 'item', id: 'a' }], 'copy')
  assert.equal(afterPaste(cut, true), null)
  assert.deepEqual(afterPaste(copy, true), copy)
  assert.deepEqual(afterPaste(cut, false), cut)
  assert.deepEqual(afterPaste(copy, false), copy)
  assert.equal(afterPaste(null, true), null)
})

test('a paste cannot settle a newer Ablage or run twice at once', () => {
  const first = fillAblage([{ kind: 'item', id: 'a' }], 'cut')
  const operation = beginPaste(first, null)
  assert.ok(operation)
  assert.equal(beginPaste(first, operation), null)

  const newer = fillAblage([{ kind: 'item', id: 'b' }], 'cut')
  assert.deepEqual(settlePaste(newer, operation, operation, true), {
    ablage: newer,
    pending: null,
  })
})

test('only a cut Ablage dims its rows', () => {
  const targets = [
    { kind: 'item' as const, id: 'a' },
    { kind: 'group' as const, id: 'work' },
  ]
  assert.deepEqual([...cutIds(fillAblage(targets, 'cut'))].sort(), [
    'a',
    'work',
  ])
  assert.equal(cutIds(fillAblage(targets, 'copy')).size, 0)
  assert.equal(cutIds(null).size, 0)
})

test('a folder cannot go into itself or below itself', () => {
  const work = [{ kind: 'group' as const, id: 'work' }]
  assert.equal(wouldCycle(work, GROUPS, 'work'), true)
  assert.equal(wouldCycle(work, GROUPS, 'server'), true)
  assert.equal(wouldCycle(work, GROUPS, 'db'), true)
  assert.equal(wouldCycle(work, GROUPS, 'home'), false)
  assert.equal(wouldCycle(work, GROUPS, null), false)
  // Entries never make a cycle, even with the id of a folder as target.
  assert.equal(wouldCycle([{ kind: 'item', id: 'x' }], GROUPS, 'db'), false)
  // One folder of several is enough.
  assert.equal(
    wouldCycle(
      [
        { kind: 'group', id: 'home' },
        { kind: 'group', id: 'server' },
      ],
      GROUPS,
      'db',
    ),
    true,
  )
})

test('targets deleted meanwhile are split off and counted', () => {
  const result = splitMissing(
    [
      { kind: 'item', id: 'a' },
      { kind: 'item', id: 'gone' },
      { kind: 'group', id: 'work' },
      { kind: 'group', id: 'gone-folder' },
    ],
    { itemIds: new Set(['a', 'b']), groupIds: new Set(['work']) },
  )
  assert.deepEqual(result.present, [
    { kind: 'item', id: 'a' },
    { kind: 'group', id: 'work' },
  ])
  assert.equal(result.missing, 2)
})

test('an id is looked up under its own kind', () => {
  // A folder id in the item set (or the other way) does not count as present.
  const result = splitMissing([{ kind: 'group', id: 'a' }], {
    itemIds: new Set(['a']),
    groupIds: new Set(),
  })
  assert.deepEqual(result, { present: [], missing: 1 })
})
