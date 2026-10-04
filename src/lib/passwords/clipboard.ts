// The Ablage of the password manager (spec 036, FR-013, FR-014, FR-021, research R10): entries and
// folders put aside by Ausschneiden or Kopieren until Einfügen. It holds ids and the mode only,
// never a title or a value, and it never touches the clipboard of the operating system (that one
// belongs to single values, FR-017). Pure, so `scripts/check-passwords-clipboard.ts` runs it
// without vue.
import { descendantIds, type TreeGroup } from './tree.ts'

/** An entry or a folder by id; the same shape as the binding `Target`. */
export type Target = { kind: 'item' | 'group'; id: string }

export type AblageMode = 'cut' | 'copy'

export type Ablage = { targets: readonly Target[]; mode: AblageMode } | null

export type PasteOperation = { ablage: Exclude<Ablage, null> }

/** The selection as targets: an id that names a folder is a folder, every other id an entry. */
export function toTargets(
  ids: readonly string[],
  groupIds: ReadonlySet<string>,
): Target[] {
  return ids.map((id) => ({ kind: groupIds.has(id) ? 'group' : 'item', id }))
}

/** What Ausschneiden or Kopieren leaves in the Ablage: it replaces what was there. */
export function fillAblage(
  targets: readonly Target[],
  mode: AblageMode,
): Ablage {
  const seen = new Set<string>()
  const unique = targets.filter((target) => {
    const key = `${target.kind}:${target.id}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
  return unique.length > 0 ? { targets: unique, mode } : null
}

/** Starts one cut paste unless another paste is still in flight. */
export function beginPaste(
  ablage: Ablage,
  pending: PasteOperation | null,
): PasteOperation | null {
  if (pending !== null || ablage?.mode !== 'cut') return null
  return { ablage }
}

/** The Ablage after Einfügen: a cut that worked is done, a copy can be pasted again, and after a
 * failure everything stays. */
export function afterPaste(ablage: Ablage, succeeded: boolean): Ablage {
  if (ablage === null) return null
  return succeeded && ablage.mode === 'cut' ? null : ablage
}

/** Finishes a paste without consuming a newer Ablage that replaced its operation. */
export function settlePaste(
  current: Ablage,
  pending: PasteOperation | null,
  operation: PasteOperation,
  succeeded: boolean,
): { ablage: Ablage; pending: PasteOperation | null } {
  if (pending !== operation) return { ablage: current, pending }
  return {
    ablage:
      current === operation.ablage
        ? afterPaste(operation.ablage, succeeded)
        : current,
    pending: null,
  }
}

/** The ids the list and the tree dim: those of a cut Ablage. */
export function cutIds(ablage: Ablage): Set<string> {
  return ablage?.mode === 'cut'
    ? new Set(ablage.targets.map((target) => target.id))
    : new Set()
}

/** Whether moving `targets` into `to` would put a folder into itself or one of its subfolders
 * (`to === null` is the top level, which never is). */
export function wouldCycle(
  targets: readonly Target[],
  groups: readonly TreeGroup[],
  to: string | null,
): boolean {
  if (to === null) return false
  return targets.some(
    (target) =>
      target.kind === 'group' &&
      (target.id === to || descendantIds(groups, target.id).includes(to)),
  )
}

/** The targets that still exist, and how many were deleted meanwhile (on this or another device). */
export function splitMissing(
  targets: readonly Target[],
  existing: { itemIds: ReadonlySet<string>; groupIds: ReadonlySet<string> },
): { present: Target[]; missing: number } {
  const present = targets.filter((target) =>
    target.kind === 'group'
      ? existing.groupIds.has(target.id)
      : existing.itemIds.has(target.id),
  )
  return { present, missing: targets.length - present.length }
}
