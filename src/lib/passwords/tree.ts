// Folder tree of the password manager (spec 034-password-manager, FR-009, FR-015): which folders
// make up the trash. The tree builder itself comes with the folders story.

/** The part of a folder row the tree needs. */
export type TreeGroup = { id: string; parentId: string | null }

/** The id of the trash folder (data-model.md): a row with a fixed id. */
export const TRASH_GROUP_ID = 'trash'

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
