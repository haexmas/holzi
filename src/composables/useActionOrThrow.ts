/**
 * Like `useAction`, for controls that already handle errors with
 * `try`/`catch` (spec 020-tab-navigation, T052): resolves the action's result
 * and, on failure, rethrows the handler's original error (or an `Error` with
 * the outcome message), so existing structured error parsing — such as
 * HuggingFace error keys — keeps working unchanged.
 */
export function useActionOrThrow(id: string) {
  const shell = useShellStore()
  return async (input: Record<string, unknown> = {}): Promise<unknown> => {
    const outcome = await shell.runAction(id, input, { kind: 'user' })
    if (outcome.ok) return outcome.result
    throw outcome.error !== undefined
      ? outcome.error
      : new Error(outcome.message)
  }
}
