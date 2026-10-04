// Part of `pnpm check:passwords` (spec 034-password-manager, US2, FR-009, FR-015): the folder tree
// (src/lib/passwords/tree.ts). It nests folders, orders siblings, puts entries with an unknown
// folder at the root and keeps the trash apart.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  buildTree,
  descendantIds,
  findNode,
  flattenFolders,
  groupPath,
  isInTrash,
  moveBefore,
  moveDown,
  moveUp,
  trashGroupIds,
  type TreeGroup,
} from '../src/lib/passwords/tree.ts'

function group(
  id: string,
  name: string | null,
  parentId: string | null = null,
  sortOrder: number | null = null,
): TreeGroup {
  return { id, name, parentId, sortOrder }
}

const GROUPS: TreeGroup[] = [
  group('work', 'Work'),
  group('home', 'home'),
  group('servers', 'Servers', 'work'),
  group('apps', 'Apps', 'work', 1),
  group('dbs', 'Databases', 'work', 0),
  group('trash', null),
  group('old', 'Old stuff', 'trash'),
  group('older', 'Older', 'old'),
]

test('folders nest by parent and the trash subtree is kept apart', () => {
  const tree = buildTree(GROUPS, [])
  assert.deepEqual(
    tree.roots.map((node) => node.group.id),
    ['home', 'work'],
    'the trash is not a root folder',
  )
  const work = tree.roots.find((node) => node.group.id === 'work')!
  assert.deepEqual(
    work.children.map((node) => node.group.id),
    ['dbs', 'servers', 'apps'],
    'sort order first (an empty one counts as 0 and sorts by name with the 0s), then name',
  )
  assert.deepEqual(tree.trash.groups.map((g) => g.id).sort(), [
    'old',
    'older',
    'trash',
  ])
})

test('siblings sort by sort order, then by name ignoring case', () => {
  const tree = buildTree(
    [
      group('b', 'banana'),
      group('a', 'Apple'),
      group('c', 'cherry', null, -1),
      group('d', 'Äpfel', null, 5),
    ],
    [],
  )
  assert.deepEqual(
    tree.roots.map((node) => node.group.id),
    ['c', 'a', 'b', 'd'],
  )
})

test('entries without a known folder are at the root, and entries are counted per folder', () => {
  const tree = buildTree(GROUPS, [
    { groupId: null },
    { groupId: '' },
    { groupId: 'missing' },
    { groupId: 'work' },
    { groupId: 'work' },
    { groupId: 'servers' },
    { groupId: 'older' },
    { groupId: 'trash' },
  ])
  assert.equal(tree.rootItemCount, 3)
  const work = tree.roots.find((node) => node.group.id === 'work')!
  assert.equal(work.itemCount, 2, 'only the entries directly in the folder')
  assert.equal(
    work.children.find((n) => n.group.id === 'servers')!.itemCount,
    1,
  )
  assert.equal(tree.trash.itemCount, 2, 'everything in the trash and below it')
})

test('a folder whose parent is missing is shown at the root', () => {
  const tree = buildTree([group('lost', 'Lost', 'gone')], [])
  assert.deepEqual(
    tree.roots.map((n) => n.group.id),
    ['lost'],
  )
})

test('descendantIds never contains the folder itself', () => {
  assert.deepEqual(descendantIds(GROUPS, 'work').sort(), [
    'apps',
    'dbs',
    'servers',
  ])
  assert.deepEqual(descendantIds(GROUPS, 'servers'), [])
  assert.deepEqual(descendantIds(GROUPS, 'unknown'), [])
  // A loop in corrupt data ends and still leaves the folder out.
  const looped = [group('x', 'X', 'y'), group('y', 'Y', 'x')]
  assert.deepEqual(descendantIds(looped, 'x'), ['y'])
})

test('trashGroupIds and isInTrash find the trash and everything below it', () => {
  const ids = trashGroupIds(GROUPS)
  assert.deepEqual([...ids].sort(), ['old', 'older', 'trash'])
  assert.equal(isInTrash('older', ids), true)
  assert.equal(isInTrash('work', ids), false)
  assert.equal(isInTrash(null, ids), false)
})

test('groupPath names the folders from the top, with an empty name shown as the trash', () => {
  assert.deepEqual(groupPath(GROUPS, 'servers'), ['Work', 'Servers'])
  assert.deepEqual(groupPath(GROUPS, null), [])
  assert.deepEqual(groupPath(GROUPS, 'missing'), [])
  assert.deepEqual(groupPath(GROUPS, 'older'), [null, 'Old stuff', 'Older'])
})

test('moving a folder up, down or before a sibling reorders the ids', () => {
  const ids = ['a', 'b', 'c', 'd']
  assert.deepEqual(moveUp(ids, 'c'), ['a', 'c', 'b', 'd'])
  assert.deepEqual(moveUp(ids, 'a'), ids)
  assert.deepEqual(moveDown(ids, 'b'), ['a', 'c', 'b', 'd'])
  assert.deepEqual(moveDown(ids, 'd'), ids)
  assert.deepEqual(moveBefore(ids, 'd', 'b'), ['a', 'd', 'b', 'c'])
  assert.deepEqual(moveBefore(ids, 'a', 'c'), ['b', 'a', 'c', 'd'])
  assert.deepEqual(moveBefore(ids, 'a', 'a'), ids)
  assert.deepEqual(moveBefore(ids, 'x', 'a'), ids)
  assert.deepEqual(moveUp(ids, 'zzz'), ids)
  // The inputs are never changed.
  assert.deepEqual(ids, ['a', 'b', 'c', 'd'])
})

test('flattenFolders lists the folders in display order with their depth', () => {
  const tree = buildTree(GROUPS, [])
  assert.deepEqual(flattenFolders(tree.roots), [
    { id: 'home', name: 'home', depth: 0 },
    { id: 'work', name: 'Work', depth: 0 },
    { id: 'dbs', name: 'Databases', depth: 1 },
    { id: 'servers', name: 'Servers', depth: 1 },
    { id: 'apps', name: 'Apps', depth: 1 },
  ])
})

test('a node is found at any depth, not in the trash (spec 036)', () => {
  const tree = buildTree(GROUPS, [])
  assert.equal(findNode(tree.roots, 'servers')?.group.name, 'Servers')
  assert.equal(findNode(tree.roots, 'work')?.children.length, 3)
  assert.equal(findNode(tree.roots, 'trash'), undefined)
  assert.equal(findNode(tree.roots, 'nope'), undefined)
})
