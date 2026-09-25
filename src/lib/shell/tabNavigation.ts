// Store-level tab navigation (spec 020-tab-navigation, T014, research R2/R11/R12): keeps one
// history per open tab in sync with the layout and opens apps or tabs at a location. The layout
// reducers stay unchanged; these helpers run them and then tell a new tab (fresh history at the
// location) from a re-activated singleton (push the location onto its history). Histories live
// next to the store's non-persisted tab runtime, never in the persisted layout (FR-011).
import type { ShellAppDefinition } from './apps.ts'
import { findOpenTab, openApp } from './layoutState.ts'
import {
  createHistory,
  go,
  push,
  replace,
  type TabHistory,
  type TabLocation,
} from './navigation.ts'
import { addTab } from './tabs.ts'
import type { ShellState } from './types.ts'

export type TabHistories = Map<string, TabHistory>

/** Title the tab shows right now, stored on the entry it leaves (research R9). */
export type LeavingTitle = (tabId: string) => string | null

export type OpenResult = { tabId: string | null; created: boolean }

function tabIds(state: ShellState): Set<string> {
  return new Set(state.windows.flatMap((w) => w.tabs.map((t) => t.id)))
}

/** Drops histories of closed tabs and gives every new tab a fresh `/` history. */
export function syncHistories(
  histories: TabHistories,
  state: ShellState,
): void {
  const live = tabIds(state)
  for (const id of [...histories.keys()]) {
    if (!live.has(id)) histories.delete(id)
  }
  for (const id of live) {
    if (!histories.has(id)) histories.set(id, createHistory())
  }
}

function settle(
  state: ShellState,
  histories: TabHistories,
  before: Set<string>,
  appId: string,
  at: string | TabLocation | null,
  leavingTitle: LeavingTitle,
): OpenResult {
  const created = [...tabIds(state)].find((id) => !before.has(id))
  if (created) {
    histories.set(created, createHistory(at ?? '/'))
    syncHistories(histories, state)
    return { tabId: created, created: true }
  }
  const existing = findOpenTab(state, appId)?.tab.id ?? null
  syncHistories(histories, state)
  if (existing && at !== null) {
    const history = histories.get(existing)
    if (history)
      histories.set(existing, push(history, at, leavingTitle(existing)))
  }
  return { tabId: existing, created: false }
}

/** `openApp` (015 FR-016) plus a start location (FR-012). */
export function openAppAt(
  state: ShellState,
  histories: TabHistories,
  appId: string,
  at: string | TabLocation | null,
  apps: readonly ShellAppDefinition[],
  leavingTitle: LeavingTitle = () => null,
): OpenResult {
  const before = tabIds(state)
  openApp(state, appId, apps)
  return settle(state, histories, before, appId, at, leavingTitle)
}

/** `addTab` (015 FR-032/FR-033) plus a start location (FR-012). */
export function addTabAt(
  state: ShellState,
  histories: TabHistories,
  windowId: string,
  appId: string,
  at: string | TabLocation | null,
  apps: readonly ShellAppDefinition[],
  leavingTitle: LeavingTitle = () => null,
): OpenResult {
  const before = tabIds(state)
  addTab(state, windowId, appId, apps)
  return settle(state, histories, before, appId, at, leavingTitle)
}

/** Push (FR-004) or replace (FR-005) on one tab's history; `true` if it changed. */
export function navigateTab(
  histories: TabHistories,
  tabId: string,
  to: string | TabLocation,
  options: { replace?: boolean; leavingTitle?: string | null } = {},
): boolean {
  const history = histories.get(tabId)
  if (!history) return false
  const next = options.replace
    ? replace(history, to)
    : push(history, to, options.leavingTitle ?? null)
  if (next === history) return false
  histories.set(tabId, next)
  return true
}

/** Back/forward/jump on one tab's history (FR-007); `true` if it moved. */
export function goTab(
  histories: TabHistories,
  tabId: string,
  delta: number,
  leavingTitle: string | null = null,
): boolean {
  const history = histories.get(tabId)
  if (!history) return false
  const next = go(history, delta, leavingTitle)
  if (next === history) return false
  histories.set(tabId, next)
  return true
}
