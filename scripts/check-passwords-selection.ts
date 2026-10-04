// Part of `pnpm check:passwords` (spec 034-password-manager, FR-012): the multi-selection of
// entries (src/lib/passwords/selection.ts).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  draggedIds,
  itemsPayload,
  parseItemsPayload,
} from '../src/lib/passwords/dnd.ts'
import {
  emptySelection,
  isSelected,
  pruneSelection,
  selectAll,
  selectAllState,
  selectRange,
  toggle,
} from '../src/lib/passwords/selection.ts'

const VISIBLE = ['a', 'b', 'c', 'd', 'e']

test('toggle adds, removes and moves the anchor', () => {
  let state = toggle(emptySelection(), 'b')
  assert.deepEqual(state.selected, ['b'])
  assert.equal(state.anchor, 'b')
  state = toggle(state, 'd')
  assert.deepEqual(state.selected, ['b', 'd'])
  state = toggle(state, 'b')
  assert.deepEqual(state.selected, ['d'])
  assert.equal(isSelected(state, 'd'), true)
  assert.equal(isSelected(state, 'b'), false)
})

test('a range runs from the anchor to the target in either direction and keeps the selection', () => {
  const state = toggle(emptySelection(), 'b')
  assert.deepEqual(selectRange(state, VISIBLE, 'd').selected, ['b', 'c', 'd'])
  const backwards = toggle(emptySelection(), 'd')
  assert.deepEqual(selectRange(backwards, VISIBLE, 'b').selected, [
    'd',
    'b',
    'c',
  ])
  // The anchor stays, so a second range grows from the same place.
  const grown = selectRange(selectRange(state, VISIBLE, 'c'), VISIBLE, 'e')
  assert.deepEqual(grown.selected, ['b', 'c', 'd', 'e'])
  assert.equal(grown.anchor, 'b')
})

test('a range without an anchor, or to a hidden entry, does the safe thing', () => {
  assert.deepEqual(selectRange(emptySelection(), VISIBLE, 'c').selected, ['c'])
  const state = toggle(emptySelection(), 'b')
  assert.equal(selectRange(state, VISIBLE, 'zzz'), state)
  const hiddenAnchor = toggle(emptySelection(), 'x')
  assert.deepEqual(selectRange(hiddenAnchor, VISIBLE, 'c').selected, ['x', 'c'])
})

test('select all and prune', () => {
  const all = selectAll(VISIBLE)
  assert.deepEqual(all.selected, VISIBLE)
  assert.equal(selectAll([]).anchor, null)
  const pruned = pruneSelection(all, new Set(['a', 'c']))
  assert.deepEqual(pruned.selected, ['a', 'c'])
  assert.equal(pruned.anchor, null, 'the anchor e is gone')
  assert.equal(
    pruneSelection(all, new Set(VISIBLE)),
    all,
    'nothing changes, same object',
  )
})

test('a drag carries the selection when the dragged entry is part of it', () => {
  assert.deepEqual(draggedIds('b', ['a', 'b']), ['a', 'b'])
  assert.deepEqual(draggedIds('c', ['a', 'b']), ['c'])
  assert.deepEqual(draggedIds('c', []), ['c'])
})

test('the drag payload round trips ids and refuses anything else', () => {
  assert.deepEqual(parseItemsPayload(itemsPayload(['a', 'b'])), ['a', 'b'])
  assert.deepEqual(parseItemsPayload(''), [])
  assert.deepEqual(parseItemsPayload(undefined), [])
  assert.deepEqual(parseItemsPayload('not json'), [])
  assert.deepEqual(parseItemsPayload('{"a":1}'), [])
  assert.deepEqual(parseItemsPayload('[1,2]'), [])
  assert.deepEqual(parseItemsPayload('["a",""]'), [])
})

test('the select-all box is empty, partly filled or full (spec 036, FR-012)', () => {
  assert.equal(selectAllState([], VISIBLE), 'none')
  assert.equal(selectAllState(['b'], VISIBLE), 'some')
  assert.equal(selectAllState([...VISIBLE].reverse(), VISIBLE), 'all')
  // What no longer shows does not count: all visible ones selected is "all".
  assert.equal(selectAllState([...VISIBLE, 'gone'], VISIBLE), 'all')
  assert.equal(selectAllState(['gone'], VISIBLE), 'none')
  assert.equal(selectAllState([], []), 'none')
})

test('a drag of a selected folder row carries the selection with its entries (spec 036, FR-020)', () => {
  assert.deepEqual(draggedIds('folder', ['a', 'folder']), ['a', 'folder'])
})
