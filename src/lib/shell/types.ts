// Pure data types for the Workspace-Shell (spec 015-workspace-shell, T015).
// No Nuxt auto-imports: this module (and its siblings under src/lib/shell/)
// must load standalone under `node scripts/check-shell-state.ts`
// (plan research R6) — import other `./*.ts` siblings with an explicit
// extension, never a `~/` alias.

/** A geometric size, used for an app's default/minimum window size. */
export type Size = {
  width: number
  height: number
}

/** Below this Shell area width, the compact (full-area window) presentation applies (research
 * R7, spec.md Assumptions: a common tablet breakpoint). Shared by `layoutState.ts` (hydrate) and
 * `geometry.ts` (T027, live resize). */
export const COMPACT_MAX_WIDTH = 767

/** An ordered container for windows on one device (data-model.md `workspaces`). No stored name:
 * the UI derives "Arbeitsbereich N" from `position` (spec FR-019). */
export type Workspace = {
  id: string
  position: number
}

/** An opened app instance in exactly one window. `appId` is opaque (research R5/R9): unknown to
 * this feature's app registry is valid on the wire, so a future `extension.*` id needs no schema
 * change; `hydrate` drops a tab whose `appId` the registry does not resolve (FR-025). */
export type ShellTab = {
  id: string
  appId: string
}

/** A frame with a title bar in exactly one workspace, holding one or more tabs. `x`/`y`/`width`/
 * `height` are the normal (non-maximized) geometry — maximizing never overwrites them (research
 * R7). `stack` is this device's window stacking rank (higher = further front). */
export type ShellWindow = {
  id: string
  workspaceId: string
  x: number
  y: number
  width: number
  height: number
  minimized: boolean
  maximized: boolean
  stack: number
  tabs: ShellTab[]
  activeTabId: string
}

/** The Shell's persisted-shape in-memory state for one device (data-model.md "Frontend-Zustand"). */
export type ShellState = {
  workspaces: Workspace[]
  windows: ShellWindow[]
  activeWorkspaceId: string
  /** The focused window in the active workspace; `null` when no window is open there. */
  activeWindowId: string | null
  /** Monotonically increasing; the source of the next `stack` value on focus/open. */
  nextStack: number
  /** The Shell's visible drawing area (the resizable region windows are clamped into). */
  area: Size
  /** Derived from `area.width <= COMPACT_MAX_WIDTH` (research R7); never written directly. */
  compact: boolean
}

/** What a `registerCloseGuard` callback (contracts/shell-app-contract.md) returns when the Shell
 * should ask the user before closing the tab; `null` means it may close without asking. */
export type CloseGuardResult = {
  /** i18n key of the reason shown in the confirmation (e.g. `shell.close.activeReply`). */
  reasonKey: string
  /** Runs after the user confirms; resolves once the tab may close safely. */
  confirmAsync: () => Promise<void>
}

export type CloseGuard = () => CloseGuardResult | null

/** Never persisted (data-model.md): per-tab runtime bookkeeping the store keeps alongside the
 * persisted `ShellTab`, keyed by tab id. */
export type TabRuntime = {
  attention: boolean
  titleOverride: string | null
  guard: CloseGuard | null
  mounted: boolean
}

/** The persisted part of one device's layout — shaped like the eventual `ShellLayoutDto`
 * (contracts/tauri-commands.md, wired in T048) so `hydrate` does not need to change shape once
 * that DTO exists; until Phase 7, callers pass an empty layout (no workspaces, no windows). */
export type PersistedLayout = {
  workspaces: Workspace[]
  windows: ShellWindow[]
  activeWorkspaceId: string
}
