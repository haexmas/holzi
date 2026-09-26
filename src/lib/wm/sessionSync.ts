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

/** The result of the `settings.sessionRestore.*` actions: a value that is not
 * set is left out, because the action schema subset has no `null` (spec 020
 * research R8) and the schema becomes an agent tool description in spec 021. */
export type RestoreStateResult = {
  device?: boolean
  vault?: boolean
  effective: boolean
}

export function toRestoreResult(state: RestoreState): RestoreStateResult {
  const result: RestoreStateResult = { effective: state.effective }
  if (state.device !== null) result.device = state.device
  if (state.vault !== null) result.vault = state.vault
  return result
}

export function fromRestoreResult(result: RestoreStateResult): RestoreState {
  return {
    device: result.device ?? null,
    vault: result.vault ?? null,
    effective: result.effective,
  }
}

/** The one choice the settings view offers (spec 022 FR-004). */
export type RestoreChoice = 'off' | 'device' | 'vault'

export type RestoreStep = { scope: RestoreScope; enabled: boolean | null }

/** Which choice the settings view shows for a restore state. */
export function restoreChoice(state: RestoreState): RestoreChoice {
  if (!state.effective) return 'off'
  return state.device === true && state.vault !== true ? 'device' : 'vault'
}

/**
 * The writes that turn `state` into `choice`, in an order that never makes
 * restore apply in between when the choice keeps it on. "Off" while the vault
 * value is on only turns this device off (`device: false`): other devices keep
 * their session. "Only this device" clears a vault value that is on, because
 * otherwise the view would still show "all devices".
 */
export function restoreChoiceSteps(
  state: RestoreState,
  choice: RestoreChoice,
): RestoreStep[] {
  const steps: RestoreStep[] = []
  if (choice === 'vault') {
    if (state.vault !== true) steps.push({ scope: 'vault', enabled: true })
    if (state.device !== null) steps.push({ scope: 'device', enabled: null })
  } else if (choice === 'device') {
    if (state.device !== true) steps.push({ scope: 'device', enabled: true })
    if (state.vault === true) steps.push({ scope: 'vault', enabled: null })
  } else if (state.vault === true) {
    if (state.device !== false) steps.push({ scope: 'device', enabled: false })
  } else if (state.device === true) {
    steps.push({ scope: 'device', enabled: null })
  }
  return steps
}

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
