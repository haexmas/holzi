// Saving and restoring the window manager session (spec 022-session-restore, research R7,
// contracts/wm-session.md §4). Pure: the store (`stores/windowManager.ts`) passes its reactive
// state, the history map and the save queue (`composables/useWmSession.ts`), so this runs under
// `node scripts/check-wm-session.ts` without Vue or Pinia.
import type { AppDefinition } from './apps.ts'
import { hydrate } from './layoutState.ts'
import type { TabHistory } from './navigation.ts'
import {
  parseWmSession,
  snapshotSession,
  splitSession,
  type WmSession,
} from './session.ts'
import type { WmState } from './types.ts'

/** What `wm_session_restore_get`/`_set` return (mirrors `SessionRestoreState` in
 * `src/types/bindings/`). */
export type RestoreState = {
  device: boolean | null
  vault: boolean | null
  effective: boolean
}

export type RestoreScope = 'device' | 'vault'

/** The part of `useWmSession()` this module drives. */
export type SessionPort = {
  saveNow(session: WmSession): void
  saveSoon(session: WmSession): void
  load(): Promise<{ restore: RestoreState; session: unknown }>
  setRestore(
    scope: RestoreScope,
    enabled: boolean | null,
  ): Promise<RestoreState>
  getRestore(): Promise<RestoreState>
  flushAsync(): Promise<void>
}

export function createSessionSync(deps: {
  state: WmState
  histories: Map<string, TabHistory>
  apps: readonly AppDefinition[]
  port: SessionPort
  /** Re-syncs per-tab runtime and histories after `state` was replaced. */
  onRestored: () => void
}) {
  const { state, histories, port } = deps
  /** Whether the setting applies on this device; `false` until restored (FR-003). */
  let enabled = false

  function snapshot(): WmSession {
    return snapshotSession(state, (tabId) => histories.get(tabId))
  }

  /** Saves at once (structural changes); does nothing while the setting does not apply. */
  function saveNow(): void {
    if (enabled) port.saveNow(snapshot())
  }

  /** Saves debounced (geometry, focus, navigation); does nothing while the setting does not
   * apply. */
  function saveSoon(): void {
    if (enabled) port.saveSoon(snapshot())
  }

  function replaceState(session: WmSession | null): void {
    const { layout, histories: saved } = session
      ? splitSession(session)
      : {
          layout: { workspaces: [], windows: [], activeWorkspaceId: '' },
          histories: new Map<string, TabHistory>(),
        }
    Object.assign(state, hydrate(layout, deps.apps, state.area))
    histories.clear()
    for (const [tabId, history] of saved) histories.set(tabId, history)
    deps.onRestored()
  }

  /** Starts the vault session: restores the saved session if the setting applies and one is
   * stored, otherwise one empty workspace. Never throws (FR-012): a failed load or an invalid
   * session is logged and holzi starts empty. */
  async function restoreAsync(): Promise<void> {
    let loaded: { restore: RestoreState; session: unknown }
    try {
      loaded = await port.load()
    } catch (error) {
      console.error('[wm] wm_session_load failed; starting empty', error)
      enabled = false
      replaceState(null)
      return
    }
    enabled = loaded.restore.effective
    const session =
      loaded.session == null ? null : parseWmSession(loaded.session)
    if (loaded.session != null && session === null)
      console.error('[wm] the saved session is not valid; starting empty')
    replaceState(enabled ? session : null)
  }

  /** Takes over a new setting (after `wm_session_restore_set`). Turning it on saves the current
   * session right away (FR-005); turning it off stops saving, the backend already deleted the
   * saved session (FR-007). */
  function applyRestore(restore: RestoreState): void {
    const wasEnabled = enabled
    enabled = restore.effective
    if (enabled && !wasEnabled) saveNow()
  }

  /** Sets (`true`/`false`) or resets (`null`) one scope's value through the same queue as the
   * saves, so no earlier save can land after it, and takes over the result. */
  async function setRestoreAsync(
    scope: RestoreScope,
    enabled: boolean | null,
  ): Promise<RestoreState> {
    const restore = await port.setRestore(scope, enabled)
    applyRestore(restore)
    return restore
  }

  return {
    saveNow,
    saveSoon,
    restoreAsync,
    applyRestore,
    setRestoreAsync,
    getRestoreAsync: () => port.getRestore(),
    flushAsync: () => port.flushAsync(),
    isEnabled: () => enabled,
  }
}
