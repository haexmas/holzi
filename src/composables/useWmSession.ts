import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { withoutHistories, type WmSession } from '../lib/wm/session.ts'
// Mirrors `SessionRestoreState`/`WmSessionLoad` in `src/types/bindings/`; those import each other
// without file extensions, which the Node-run check scripts cannot load.
import type { RestoreState as SessionRestoreState } from '../lib/wm/sessionSync.ts'

type WmSessionLoad = { restore: SessionRestoreState; session: unknown }

/**
 * Narrowed, non-generic shape of Tauri's `invoke`, so a plain
 * `async (cmd, args) => ...` fake satisfies it in tests. Callers cast the
 * result at each call site.
 */
type WmInvokeFn = (
  cmd: string,
  args?: Record<string, unknown>,
) => Promise<unknown>

export type SessionRestoreScope = 'device' | 'vault'

const SAVE_DEBOUNCE_MS = 400

function isTooLarge(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    (error as { kind?: unknown }).kind === 'SessionTooLarge'
  )
}

/**
 * The saving side of the opt-in session restore (spec 022-session-restore,
 * contracts/wm-session.md §1/§4): a serialized invoke queue, so a slow save
 * can never overtake a later one, a 400 ms debounce for continuous changes,
 * and a retry that keeps the newest unsaved snapshot pending after a
 * failure. Only the latest snapshot matters, so there is one pending slot,
 * not a list.
 *
 * A snapshot above the backend's size limit is retried once with every tab
 * history reduced to its current entry, and dropped if it is still too
 * large (data-model.md "Größe"), instead of being retried forever.
 *
 * Whether saving applies at all is the store's business
 * (`stores/windowManager.ts` only calls `saveNow`/`saveSoon` while the
 * setting applies); the backend checks again, so a save that arrives after
 * the setting was turned off writes nothing.
 *
 * `invokeFn` defaults to the real Tauri `invoke`;
 * `scripts/check-wm-persistence.ts` substitutes a fake.
 */
export function useWmSession(invokeFn: WmInvokeFn = tauriInvoke) {
  /** Never rejects, so the queue keeps flowing after a failure; each
   * operation's own promise still carries its failure to the caller. */
  let queueTail: Promise<unknown> = Promise.resolve()
  let pending: WmSession | null = null
  let debounceTimer: ReturnType<typeof setTimeout> | null = null

  function runExclusive<T>(op: () => Promise<T>): Promise<T> {
    const started = queueTail.then(op, op)
    queueTail = started.then(
      () => undefined,
      () => undefined,
    )
    return started
  }

  function clearDebounce(): void {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }
  }

  function send(session: WmSession): Promise<unknown> {
    return invokeFn('wm_session_save', { args: { session } })
  }

  /** Sends the pending snapshot. Never throws: a failed save stays pending
   * unless a newer snapshot replaced it meanwhile. */
  async function flushPending(): Promise<void> {
    const session = pending
    if (session === null) return
    try {
      try {
        await send(session)
      } catch (error) {
        if (!isTooLarge(error)) throw error
        console.warn(
          '[wm] session too large to save; retrying without tab histories',
        )
        try {
          await send(withoutHistories(session))
        } catch (retryError) {
          if (!isTooLarge(retryError)) throw retryError
          console.error(
            '[wm] session too large even without tab histories; not saved',
          )
        }
      }
      if (pending === session) pending = null
    } catch (error) {
      console.error(
        '[wm] wm_session_save failed; retrying with the next save',
        error,
      )
    }
  }

  /** Saves right away: structural changes (open, close, move, workspaces). */
  function saveNow(session: WmSession): void {
    pending = session
    clearDebounce()
    void runExclusive(flushPending)
  }

  /** Saves after 400 ms without further changes: geometry, focus, navigation. */
  function saveSoon(session: WmSession): void {
    pending = session
    clearDebounce()
    debounceTimer = setTimeout(() => {
      debounceTimer = null
      void runExclusive(flushPending)
    }, SAVE_DEBOUNCE_MS)
  }

  /** The setting and, when it applies, the saved session (not yet validated). */
  function load(): Promise<WmSessionLoad> {
    return runExclusive(
      async () => (await invokeFn('wm_session_load')) as WmSessionLoad,
    )
  }

  /** Sets (`true`/`false`) or resets (`null`) one scope's value. */
  function setRestore(
    scope: SessionRestoreScope,
    enabled: boolean | null,
  ): Promise<SessionRestoreState> {
    return runExclusive(
      async () =>
        (await invokeFn('wm_session_restore_set', {
          args: { scope, enabled },
        })) as SessionRestoreState,
    )
  }

  /** Reads the setting without touching the saved session. */
  function getRestore(): Promise<SessionRestoreState> {
    return runExclusive(
      async () =>
        (await invokeFn('wm_session_restore_get')) as SessionRestoreState,
    )
  }

  /** Sends a debounced save now and waits for the queue (before locking). */
  async function flushAsync(): Promise<void> {
    if (debounceTimer !== null) {
      clearDebounce()
      void runExclusive(flushPending)
    }
    await queueTail
  }

  return { saveNow, saveSoon, load, setRestore, getRestore, flushAsync }
}
