// Folder tree of the password manager (spec 034-password-manager, FR-009, FR-015): nests the
// folders, orders siblings, counts entries per folder and keeps the trash apart. Pure, so
// `scripts/check-passwords-tree.ts` runs it without vue.

/** The part of a folder row the tree needs. */
export type TreeGroup = {
  id: string
  name: string | null
  parentId: string | null
  sortOrder: number | null
}

/** The id of the trash folder (data-model.md): a row with a fixed id and no name. */
export const TRASH_GROUP_ID = 'trash'

export type TreeNode<G extends TreeGroup = TreeGroup> = {
  group: G
  children: TreeNode<G>[]
  /** Entries directly in this folder. */
  itemCount: number
}

export type Tree<G extends TreeGroup = TreeGroup> = {
  /** The top level folders in display order; the trash is not among them. */
  roots: TreeNode<G>[]
  /** Entries at the top level, also those whose folder is empty or unknown. */
  rootItemCount: number
  trash: {
    /** The trash and every folder below it. */
    groups: G[]
    /** Entries in the trash and below it. */
    itemCount: number
  }
}

/** The ids of the trash and every folder below it. */
export function trashGroupIds(groups: readonly TreeGroup[]): Set<string> {
  const ids = new Set<string>([TRASH_GROUP_ID])
  let grew = true
  while (grew) {
    grew = false
    for (const group of groups) {
      if (group.parentId && ids.has(group.parentId) && !ids.has(group.id)) {
        ids.add(group.id)
        grew = true
      }
    }
  }
  return ids
}

/** Whether an entry or folder with this folder id lies in the trash. */
export function isInTrash(
  groupId: string | null | undefined,
  trashIds: ReadonlySet<string>,
): boolean {
  return groupId != null && trashIds.has(groupId)
}

/** Case- and accent-insensitive comparison of two names. */
function compareNames(a: string | null, b: string | null): number {
  return (a ?? '').localeCompare(b ?? '', undefined, { sensitivity: 'base' })
}

/** Display order of siblings: `sortOrder` (empty counts as 0), then name ignoring case, then id. */
function compareGroups(a: TreeGroup, b: TreeGroup): number {
  return (
    (a.sortOrder ?? 0) - (b.sortOrder ?? 0) ||
    compareNames(a.name, b.name) ||
    a.id.localeCompare(b.id)
  )
}

/** Builds the tree of the folders outside the trash and counts the entries. A folder whose parent
 * does not exist is shown at the top so it stays reachable. */
export function buildTree<G extends TreeGroup>(
  groups: readonly G[],
  items: readonly { groupId: string | null }[],
): Tree<G> {
  const trashIds = trashGroupIds(groups)
  const known = new Set(groups.map((group) => group.id))
  const live = groups.filter((group) => !trashIds.has(group.id))
  const nodes = new Map<string, TreeNode<G>>(
    live.map((group) => [group.id, { group, children: [], itemCount: 0 }]),
  )
  const roots: TreeNode<G>[] = []
  for (const group of live) {
    const node = nodes.get(group.id)!
    const parent =
      group.parentId && known.has(group.parentId)
        ? nodes.get(group.parentId)
        : undefined
    if (parent) parent.children.push(node)
    else roots.push(node)
  }
  const sortLevel = (level: TreeNode<G>[]) => {
    level.sort((a, b) => compareGroups(a.group, b.group))
    for (const node of level) sortLevel(node.children)
  }
  sortLevel(roots)

  let rootItemCount = 0
  let trashedItems = 0
  for (const item of items) {
    const id = item.groupId
    if (id && trashIds.has(id)) trashedItems += 1
    else if (id && nodes.has(id)) nodes.get(id)!.itemCount += 1
    else rootItemCount += 1
  }
  return {
    roots,
    rootItemCount,
    trash: {
      groups: groups.filter((group) => trashIds.has(group.id)),
      itemCount: trashedItems,
    },
  }
}

/** The ids of every folder below `groupId`, never `groupId` itself; a loop in the data ends. */
export function descendantIds(
  groups: readonly TreeGroup[],
  groupId: string,
): string[] {
  const found: string[] = []
  const seen = new Set<string>([groupId])
  const queue = [groupId]
  while (queue.length > 0) {
    const current = queue.shift()!
    for (const group of groups) {
      if (group.parentId === current && !seen.has(group.id)) {
        seen.add(group.id)
        found.push(group.id)
        queue.push(group.id)
      }
    }
  }
  return found
}

/** The names of the folders from the top down to `groupId`; `null` stands for the empty name of
 * the trash. Empty for the top level or an unknown folder. */
export function groupPath(
  groups: readonly TreeGroup[],
  groupId: string | null,
): (string | null)[] {
  const byId = new Map(groups.map((group) => [group.id, group]))
  if (groupId === null || !byId.has(groupId)) return []
  const path: (string | null)[] = []
  const seen = new Set<string>()
  let current: TreeGroup | undefined = byId.get(groupId)
  while (current && !seen.has(current.id)) {
    seen.add(current.id)
    path.unshift(current.name)
    current = current.parentId ? byId.get(current.parentId) : undefined
  }
  return path
}

/** `ids` with `id` one place earlier; unchanged for the first one or an unknown id. */
export function moveUp(ids: readonly string[], id: string): string[] {
  const at = ids.indexOf(id)
  if (at <= 0) return [...ids]
  const next = [...ids]
  ;[next[at - 1], next[at]] = [next[at]!, next[at - 1]!]
  return next
}

/** `ids` with `id` one place later; unchanged for the last one or an unknown id. */
export function moveDown(ids: readonly string[], id: string): string[] {
  const at = ids.indexOf(id)
  if (at === -1 || at >= ids.length - 1) return [...ids]
  const next = [...ids]
  ;[next[at], next[at + 1]] = [next[at + 1]!, next[at]!]
  return next
}

/** `ids` with `dragged` placed right before `target` (drag and drop of a folder onto a sibling);
 * unchanged when either is unknown or they are the same. */
export function moveBefore(
  ids: readonly string[],
  dragged: string,
  target: string,
): string[] {
  if (dragged === target || !ids.includes(dragged) || !ids.includes(target)) {
    return [...ids]
  }
  const rest = ids.filter((id) => id !== dragged)
  rest.splice(rest.indexOf(target), 0, dragged)
  return rest
}

/** The folders of a tree as a flat list in display order with their depth, for a picker. */
export function flattenFolders<G extends TreeGroup>(
  roots: readonly TreeNode<G>[],
): { id: string; name: string | null; depth: number }[] {
  const out: { id: string; name: string | null; depth: number }[] = []
  const walk = (nodes: readonly TreeNode<G>[], depth: number) => {
    for (const node of nodes) {
      out.push({ id: node.group.id, name: node.group.name, depth })
      walk(node.children, depth + 1)
    }
  }
  walk(roots, 0)
  return out
}
