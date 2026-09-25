import { invoke as tauriInvoke } from '@tauri-apps/api/core'

/**
 * Narrowed, non-generic shape of Tauri's `invoke` — deliberately simpler
 * than its real (generic) type so a plain `async (cmd, args) => ...` fake
 * satisfies it directly in tests (a concrete-return function is not
 * assignable to a generic one, but the real `invoke` — being more general —
 * is always assignable to this narrower shape). Callers cast its result at
 * each call site instead of relying on a passed-through type parameter.
 */
type ShellInvokeFn = (
  cmd: string,
  args?: Record<string, unknown>,
) => Promise<unknown>

/**
 * Wire types for the Shell layout commands (spec 015-workspace-shell,
 * contracts/tauri-commands.md). Hand-declared rather than imported from
 * `src/types/bindings/` — same convention as `usePreferences.ts`'s
 * `PrefScope`/`PrefScopeWire` — and kept camelCase-identical to the
 * generated `WorkspaceDto`/`TabDto`/`WindowDto`/`ShellLayoutDto`/
 * `DeleteWorkspaceResult` so a drift check can compare them.
 */
export type WorkspaceDto = {
  workspaceId: string
  position: number
}

export type TabDto = {
  tabId: string
  appId: string
}

export type WindowDto = {
  windowId: string
  workspaceId: string
  x: number
  y: number
  width: number
  height: number
  isMinimized: boolean
  isMaximized: boolean
  stackOrder: number
  activeTabId: string
  tabs: TabDto[]
}

export type ShellLayoutDto = {
  workspaces: WorkspaceDto[]
  windows: WindowDto[]
  activeWorkspaceId: string
}

export type DeleteWorkspaceResult = {
  workspaces: WorkspaceDto[]
  activeWorkspaceId: string
}

const SAVE_DEBOUNCE_MS = 400

/**
 * Owns the shell layout's persistence side: a serialized invoke queue (the
 * contract's "at most one Shell call in flight" rule — otherwise a delayed
 * `shell_save_windows` could resurrect a window a later `shell_close_windows`
 * already removed), a 400ms debounce that batches geometry/stack changes
 * into one `shell_save_windows`, and dirty-retry (a window that failed to
 * save stays pending and rides along on the next save, whichever triggers
 * it) — research.md R13.
 *
 * Structural changes (open/close, move between workspaces, workspace
 * actions) are the caller's job to trigger immediately (`saveWindowNow`/
 * `closeWindowNow`), geometry/stack changes debounced (`saveWindowDebounced`).
 * Translating between the store's `ShellWindow`/`ShellTab` and these DTOs
 * is the caller's job too (`stores/shell.ts`, T048) — this composable only
 * ever sees wire shapes.
 *
 * Call once and reuse the returned object: the queue, debounce timer and
 * dirty sets are per-call closure state, not a module-level singleton
 * (unlike `useShellCloseConfirm`'s `pending` ref) — there is exactly one
 * shell layout per device, so `stores/shell.ts` calls this once at setup.
 *
 * `invokeFn` defaults to the real Tauri `invoke` — `scripts/check-shell-
 * state.ts` (T053) substitutes a fake to test the queue/debounce/dirty-
 * retry logic standalone, without a Tauri runtime or module mocking.
 */
export function useShellLayout(invokeFn: ShellInvokeFn = tauriInvoke) {
  /** Never rejects — every operation attaches here so the queue keeps
   * flowing even after a failure; the operation's OWN returned promise
   * (from `runExclusive`) still carries that failure to its caller. */
  let queueTail: Promise<unknown> = Promise.resolve()

  const pendingSaves = new Map<string, WindowDto>()
  const pendingCloses = new Set<string>()
  let debounceTimer: ReturnType<typeof setTimeout> | null = null

  /** Runs a backend operation after earlier calls, preserving its own result or error. */
  function runExclusive<T>(op: () => Promise<T>): Promise<T> {
    const started = queueTail.then(op, op)
    queueTail = started.then(
      () => undefined,
      () => undefined,
    )
    return started
  }

  /** Cancels a scheduled save before an immediate flush or orderly shutdown. */
  function clearDebounce(): void {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }
  }

  /** Sends every currently-pending save/close in one round each. Never
   * throws — a failure leaves its entries in `pendingSaves`/`pendingCloses`
   * for the next flush to retry (research.md R13's dirty-retry). */
  async function flushPending(): Promise<void> {
    if (pendingSaves.size > 0) {
      const windows = Array.from(pendingSaves.values())
      try {
        await invokeFn('shell_save_windows', { args: { windows } })
        for (const w of windows) pendingSaves.delete(w.windowId)
      } catch (error) {
        console.error(
          '[shell] shell_save_windows failed; windows stay dirty for the next save',
          error,
        )
      }
    }
    if (pendingCloses.size > 0) {
      const windowIds = Array.from(pendingCloses)
      try {
        await invokeFn('shell_close_windows', { args: { windowIds } })
        for (const id of windowIds) pendingCloses.delete(id)
      } catch (error) {
        console.error(
          '[shell] shell_close_windows failed; windows stay pending for the next save',
          error,
        )
      }
    }
  }

  /** Restarts the timer so a burst of changes is sent in one queued flush. */
  function scheduleDebouncedFlush(): void {
    clearDebounce()
    debounceTimer = setTimeout(() => {
      debounceTimer = null
      void runExclusive(flushPending)
    }, SAVE_DEBOUNCE_MS)
  }

  /** Marks `window` dirty and flushes immediately — structural changes
   * (open, move between workspaces). Fire-and-forget: failures retry via
   * `pendingSaves`, not by rejecting back to the caller. */
  function saveWindowNow(window: WindowDto): void {
    pendingSaves.set(window.windowId, window)
    clearDebounce()
    void runExclusive(flushPending)
  }

  /** Marks `window` dirty and schedules a debounced flush — geometry/stack
   * changes (drag, resize, focus). Repeated calls within 400ms coalesce
   * into one `shell_save_windows`. */
  function saveWindowDebounced(window: WindowDto): void {
    pendingSaves.set(window.windowId, window)
    scheduleDebouncedFlush()
  }

  /** Marks `windowId` for closing and flushes immediately. Cancels any
   * still-pending save for the same id — closing wins. */
  function closeWindowNow(windowId: string): void {
    pendingSaves.delete(windowId)
    pendingCloses.add(windowId)
    clearDebounce()
    void runExclusive(flushPending)
  }

  /** Loads the device's whole layout. Result-bearing and queued like every
   * other call, but never debounced or retried — the caller awaits it. */
  function loadLayout(): Promise<ShellLayoutDto> {
    return runExclusive(
      async () => (await invokeFn('shell_load_layout')) as ShellLayoutDto,
    )
  }

  /** Creates a workspace on the backend and returns its assigned id. */
  function createWorkspace(): Promise<WorkspaceDto> {
    return runExclusive(
      async () => (await invokeFn('shell_create_workspace')) as WorkspaceDto,
    )
  }

  /** Deletes a workspace after earlier queued window changes settle. */
  function deleteWorkspace(
    workspaceId: string,
  ): Promise<DeleteWorkspaceResult> {
    return runExclusive(
      async () =>
        (await invokeFn('shell_delete_workspace', {
          args: { workspaceId },
        })) as DeleteWorkspaceResult,
    )
  }

  /** Persists the active workspace as a serialized device preference. */
  function setActiveWorkspace(workspaceId: string): Promise<void> {
    return runExclusive(() =>
      invokeFn('shell_set_active_workspace', { args: { workspaceId } }).then(
        () => undefined,
      ),
    )
  }

  /** Awaits every queued and pending operation, flushing any debounced
   * save first (FR-027, before `closeAsync()`). Best-effort: a save that
   * keeps failing does not make this reject — research.md R13 accepts
   * losing at most the last 400ms of layout changes on a crash, not on an
   * orderly close, so this always resolves once the queue is idle. */
  async function flushAsync(): Promise<void> {
    if (debounceTimer !== null) {
      clearDebounce()
      void runExclusive(flushPending)
    }
    await queueTail
  }

  return {
    loadLayout,
    createWorkspace,
    deleteWorkspace,
    setActiveWorkspace,
    saveWindowNow,
    saveWindowDebounced,
    closeWindowNow,
    flushAsync,
  }
}
