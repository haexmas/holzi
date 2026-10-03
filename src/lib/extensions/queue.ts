// The queue of permission questions of extensions (spec 017, US3, T064, contracts/permissions.md
// §Anfrage zur Laufzeit). Pure: the permissions store moves it, `scripts/check-extensions-queue.ts`
// tests it. One question is shown at a time; identical questions are one.
import type { PermissionRequestEvent } from '../../types/bindings/PermissionRequestEvent.ts'

export type PermissionQuestion = PermissionRequestEvent

function sameQuestion(a: PermissionQuestion, b: PermissionQuestion): boolean {
  return (
    a.extensionId === b.extensionId &&
    a.kind === b.kind &&
    a.action === b.action &&
    a.target === b.target
  )
}

/** Adds a question unless an identical one is already waiting. */
export function enqueue(
  queue: readonly PermissionQuestion[],
  question: PermissionQuestion,
): PermissionQuestion[] {
  return queue.some((q) => sameQuestion(q, question))
    ? [...queue]
    : [...queue, question]
}

/** Takes a question out: answered, cancelled by closing the dialog, or dropped by Rust because
 * every frame that waited for it is gone. Closing the dialog never counts as "deny". */
export function remove(
  queue: readonly PermissionQuestion[],
  requestId: string,
): PermissionQuestion[] {
  return queue.filter((q) => q.requestId !== requestId)
}

/** The question shown now. */
export function current(
  queue: readonly PermissionQuestion[],
): PermissionQuestion | undefined {
  return queue[0]
}
