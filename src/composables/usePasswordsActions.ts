import { toast } from 'vue-sonner'
import {
  splitMissing,
  toTargets,
  wouldCycle,
  type Target,
} from '~/lib/passwords/clipboard'
import { isInTrash, trashGroupIds, TRASH_GROUP_ID } from '~/lib/passwords/tree'

/**
 * The actions on entries and folders that the selection bar, the context menus, the shortcuts and
 * drag and drop share (spec 036, FR-012 to FR-020), so they cannot drift apart: Ausschneiden and
 * Kopieren into the Ablage, Einfügen, moving by id and copying a username or password. Moving runs
 * as one command (all or nothing, FR-022); targets deleted meanwhile are skipped and counted, a
 * folder that would go into itself is refused before anything moves (FR-014).
 */
export function usePasswordsActions() {
  const { t } = useI18n()
  const { errString } = useErrorString()
  const store = usePasswordsStore()
  const clipboard = usePasswordsClipboardStore()
  const { moveAsync, copyFieldAsync } = usePasswords()

  const groupIds = computed(
    () => new Set(store.groups.map((group) => group.id)),
  )

  /** The entries and folders that exist outside the trash: a target in the trash counts as deleted. */
  function live() {
    const trash = trashGroupIds(store.groups)
    return {
      itemIds: new Set(
        store.headers
          .filter((header) => !isInTrash(header.groupId, trash))
          .map((header) => header.id),
      ),
      groupIds: new Set(
        store.groups
          .filter(
            (group) => group.id !== TRASH_GROUP_ID && !trash.has(group.id),
          )
          .map((group) => group.id),
      ),
    }
  }

  function targetsOf(ids: readonly string[]): Target[] {
    return toTargets(ids, groupIds.value)
  }

  function cut(ids: readonly string[]) {
    clipboard.fill(targetsOf(ids), 'cut')
  }

  function copy(ids: readonly string[]) {
    clipboard.fill(targetsOf(ids), 'copy')
  }

  function isTargetMissing(cause: unknown): boolean {
    return (
      typeof cause === 'object' &&
      cause !== null &&
      (cause as { kind?: unknown }).kind === 'InvalidInput' &&
      (cause as { reason?: unknown }).reason === 'target_missing'
    )
  }

  /** Moves what still exists of `targets` into `to` (`null` is the top level) and reports the
   * outcome. `true` when the move ran or nothing was left to move. */
  async function moveTargetsAsync(
    targets: readonly Target[],
    to: string | null,
  ): Promise<boolean> {
    let split = splitMissing(targets, live())
    for (let attempt = 0; ; attempt += 1) {
      if (split.present.length === 0) {
        toast.error(
          t(
            'passwords.clipboard.allGone',
            { count: split.missing },
            split.missing,
          ),
        )
        return true
      }
      if (wouldCycle(split.present, store.groups, to)) {
        toast.error(t('passwords.clipboard.cycle'))
        return false
      }
      try {
        await moveAsync(split.present, to)
        break
      } catch (cause) {
        // Deleted on another device after the last reload: reload once and move the rest.
        if (attempt === 0 && isTargetMissing(cause)) {
          await store.quietReloadAsync()
          split = splitMissing(targets, live())
          continue
        }
        toast.error(errString(cause))
        return false
      }
    }
    await store.quietReloadAsync()
    const moved = split.present.length
    toast.success(
      split.missing > 0
        ? t('passwords.clipboard.movedSome', {
            moved: t('passwords.clipboard.movedCount', { count: moved }, moved),
            missing: t(
              'passwords.clipboard.missingCount',
              { count: split.missing },
              split.missing,
            ),
          })
        : t('passwords.clipboard.movedCount', { count: moved }, moved),
    )
    return true
  }

  /** Moves the entries and folders with these ids, as a drop does. */
  function moveIdsAsync(ids: readonly string[], to: string | null) {
    return moveTargetsAsync(targetsOf(ids), to)
  }

  /** Einfügen into `to`: a cut moves and empties the Ablage; on a failure it stays. */
  async function pasteAsync(to: string | null) {
    const operation = clipboard.beginPaste()
    // A copy is pasted through the copy dialog of stage 3 (FR-015), and only one paste may run at
    // a time because all password-manager windows share this Ablage.
    if (!operation) return
    let succeeded = false
    try {
      succeeded = await moveTargetsAsync(operation.ablage.targets, to)
    } finally {
      clipboard.settle(operation, succeeded)
    }
  }

  /** Copies the username or the password of an entry as the entry page does: the value never
   * passes the webview, and the clipboard setting of spec 034 clears it (FR-017). */
  async function copyValueAsync(
    itemId: string,
    field: 'username' | 'password',
  ) {
    const label = t(`passwords.fields.${field}`)
    try {
      const result = await copyFieldAsync(itemId, { kind: field })
      toast.success(
        result.clearsInSeconds === null
          ? t('passwords.copiedKept', { field: label })
          : t('passwords.copied', {
              field: label,
              seconds: result.clearsInSeconds,
            }),
      )
    } catch (cause) {
      toast.error(errString(cause))
    }
  }

  return {
    targetsOf,
    cut,
    copy,
    pasteAsync,
    moveIdsAsync,
    copyValueAsync,
  }
}
