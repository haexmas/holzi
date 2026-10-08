// The dock (spec 045): what it holds, where it stands and how it reads its two preferences. Pure — no
// Vue, no Nuxt auto-imports, relative `.ts` imports only — so `scripts/check-wm-dock.ts` loads it
// directly (rule in `types.ts`).
import { resolveAppAlias, type AppDefinition } from './apps.ts'
import type { WmWindow } from './types.ts'

/** Vault preference, synced (FR-035): the ordered entries. */
export const DOCK_ITEMS_KEY = 'dock.items'
/** Device preference, never synced (FR-036): style and position. */
export const DOCK_PLACEMENT_KEY = 'dock.placement'

export type DockControlId = 'launcher' | 'workspaces' | 'windows'

export type DockItem =
  { kind: 'control'; id: DockControlId } | { kind: 'app'; appId: string }

/** A stored item as the dock knows it: an app not resolvable on this device stays stored but is not
 * shown (FR-037). */
export type DockItemState = DockItem & { available: boolean }

export type DockEdge = 'top' | 'bottom' | 'left' | 'right'
export type DockAlign = 'start' | 'center' | 'end'
export type DockStyle = 'bar' | 'wheel'
/** Only the bar has a mode; the wheel always floats (FR-030). */
export type DockMode = 'reserved' | 'floating' | 'autohide'

export type DockPlacement = {
  style: DockStyle
  edge: DockEdge
  align: DockAlign
  mode: DockMode
}

const CONTROL_IDS: readonly DockControlId[] = [
  'launcher',
  'workspaces',
  'windows',
]
const EDGES: readonly DockEdge[] = ['top', 'bottom', 'left', 'right']
const ALIGNS: readonly DockAlign[] = ['start', 'center', 'end']
const STYLES: readonly DockStyle[] = ['bar', 'wheel']
const MODES: readonly DockMode[] = ['reserved', 'floating', 'autohide']

export const DEFAULT_DOCK_ITEMS: readonly DockItem[] = CONTROL_IDS.map(
  (id) => ({ kind: 'control', id }),
)

export const DEFAULT_DOCK_PLACEMENT: DockPlacement = {
  style: 'bar',
  edge: 'bottom',
  align: 'center',
  mode: 'reserved',
}

function isOneOf<T extends string>(
  values: readonly T[],
  value: unknown,
): value is T {
  return (
    typeof value === 'string' && (values as readonly string[]).includes(value)
  )
}

function parseJson(raw: string | null): unknown {
  if (raw === null) return null
  try {
    return JSON.parse(raw)
  } catch {
    return null
  }
}

function parseItem(value: unknown): DockItem | null {
  if (typeof value !== 'object' || value === null) return null
  const item = value as Record<string, unknown>
  if (item.kind === 'control' && isOneOf(CONTROL_IDS, item.id))
    return { kind: 'control', id: item.id }
  if (item.kind === 'app' && typeof item.appId === 'string' && item.appId)
    return { kind: 'app', appId: item.appId }
  return null
}

/** The stored entries, or `null` when the value is missing or not a list at all (FR-038: the caller
 * then shows the defaults and writes nothing). Malformed elements of a list are dropped. */
export function parseDockItems(raw: string | null): DockItem[] | null {
  const value = parseJson(raw)
  if (!Array.isArray(value)) return null
  return value.flatMap((element) => parseItem(element) ?? [])
}

/** The stored placement; every missing or invalid field falls back to its default on its own. */
export function parseDockPlacement(raw: string | null): DockPlacement {
  const value = parseJson(raw)
  const stored =
    typeof value === 'object' && value !== null
      ? (value as Record<string, unknown>)
      : {}
  return {
    style: isOneOf(STYLES, stored.style)
      ? stored.style
      : DEFAULT_DOCK_PLACEMENT.style,
    edge: isOneOf(EDGES, stored.edge)
      ? stored.edge
      : DEFAULT_DOCK_PLACEMENT.edge,
    align: isOneOf(ALIGNS, stored.align)
      ? stored.align
      : DEFAULT_DOCK_PLACEMENT.align,
    mode: isOneOf(MODES, stored.mode)
      ? stored.mode
      : DEFAULT_DOCK_PLACEMENT.mode,
  }
}

function itemKey(item: DockItem): string {
  return item.kind === 'control' ? `control:${item.id}` : `app:${item.appId}`
}

/** The entries the dock works with (data-model.md): legacy app ids mapped to their app, the first of
 * any duplicate kept (FR-039), the launcher always present (FR-006), and an app unknown on this
 * device kept as unavailable instead of dropped (FR-037). */
export function normalizeDockItems(
  items: readonly DockItem[],
  apps: readonly AppDefinition[],
): DockItemState[] {
  const seen = new Set<string>()
  const result: DockItemState[] = []
  for (const stored of items) {
    const item: DockItem =
      stored.kind === 'app'
        ? { kind: 'app', appId: resolveAppAlias(stored.appId).appId }
        : stored
    const key = itemKey(item)
    if (seen.has(key)) continue
    seen.add(key)
    result.push({
      ...item,
      available:
        item.kind === 'control' || apps.some((app) => app.id === item.appId),
    })
  }
  if (!seen.has('control:launcher'))
    result.unshift({ kind: 'control', id: 'launcher', available: true })
  return result
}

/** The value written back: every stored item, unavailable ones included, without the device-local
 * `available` flag. */
export function serializeDockItems(items: readonly DockItemState[]): string {
  return JSON.stringify(
    items.map((item): DockItem =>
      item.kind === 'control'
        ? { kind: 'control', id: item.id }
        : { kind: 'app', appId: item.appId },
    ),
  )
}

/** One open instance of an app — a tab, alone in its own window or next to other tabs (spec
 * "Begriffe"). */
export type DockInstance = {
  tabId: string
  windowId: string
  workspaceId: string
}

export type DockEntry =
  | { kind: 'control'; id: DockControlId }
  | {
      kind: 'app'
      appId: string
      /** `false` for a running app that is not in the dock's entries (FR-005). */
      pinned: boolean
      /** Its tabs in every workspace, in window order. */
      instances: DockInstance[]
    }

function instancesOf(
  appId: string,
  windows: readonly WmWindow[],
): DockInstance[] {
  return windows.flatMap((window) =>
    window.tabs
      .filter((tab) => tab.appId === appId)
      .map((tab) => ({
        tabId: tab.id,
        windowId: window.id,
        workspaceId: window.workspaceId,
      })),
  )
}

/** What the dock shows: the available entries in their order, each app with its instances. */
export function resolveDockEntries(
  items: readonly DockItemState[],
  windows: readonly WmWindow[],
): DockEntry[] {
  return items.flatMap((item): DockEntry[] => {
    if (!item.available) return []
    if (item.kind === 'control') return [{ kind: 'control', id: item.id }]
    return [
      {
        kind: 'app',
        appId: item.appId,
        pinned: true,
        instances: instancesOf(item.appId, windows),
      },
    ]
  })
}
