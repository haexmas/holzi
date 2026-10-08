import { computed, readonly, ref, type InjectionKey } from 'vue'
import { usePreferences, type PrefScope } from '~/composables/usePreferences'
import {
  DEFAULT_DOCK_ITEMS,
  DEFAULT_DOCK_PLACEMENT,
  DOCK_ITEMS_KEY,
  DOCK_PLACEMENT_KEY,
  effectivePlacement,
  normalizeDockItems,
  parseDockItems,
  parseDockPlacement,
  serializeDockItems,
  type DockControlId,
  type DockItem,
  type DockItemState,
  type DockPlacement,
} from '~/lib/wm/dock'

const VAULT: PrefScope = { kind: 'vault' }

/** Provided by `wm/Dock.vue`: an entry reports its menu or chooser opening (`true`) and closing
 * (`false`), so a hiding dock stays while one is open (FR-025). */
export const DOCK_HOLD: InjectionKey<(open: boolean) => void> =
  Symbol('dockHold')

/** One state per process: a process holds one vault (spec 013). `null` = nothing readable stored,
 * so the defaults show and nothing is written until the user changes something (FR-038). */
const storedItems = ref<DockItem[] | null>(null)
const placement = ref<DockPlacement>({ ...DEFAULT_DOCK_PLACEMENT })
let deviceUuid: string | null = null
let pendingWrites = 0
let writes: Promise<void> = Promise.resolve()

/**
 * The dock's two preferences (spec 045, research R2): the entries for the whole vault (`dock.items`,
 * synced, FR-035) and style and position for this device (`dock.placement`, FR-036). Writes run one
 * after another in the order they were asked for.
 */
export function useDock() {
  const { getPrefAsync, setPrefAsync } = usePreferences()
  const { currentDeviceInfoAsync } = useDevice()
  const wm = useWindowManagerStore()

  /** Normalized against the apps of this moment, so an extension installed or removed shows or
   * hides its entry at once (FR-037). */
  const items = computed(() =>
    normalizeDockItems(storedItems.value ?? DEFAULT_DOCK_ITEMS, wm.apps()),
  )
  /** Where the dock stands right now: the stored choice, or the bottom in compact mode
   * (FR-031, FR-032). */
  const effective = computed(() =>
    effectivePlacement(placement.value, wm.compact),
  )

  async function deviceScopeAsync(): Promise<PrefScope> {
    deviceUuid ??= (await currentDeviceInfoAsync()).vaultDeviceUuid
    return { kind: 'device', uuid: deviceUuid }
  }

  async function readAsync(): Promise<{
    items: DockItem[] | null
    placement: DockPlacement
  }> {
    const [rawItems, rawPlacement] = await Promise.all([
      getPrefAsync(VAULT, DOCK_ITEMS_KEY),
      deviceScopeAsync().then((scope) =>
        getPrefAsync(scope, DOCK_PLACEMENT_KEY),
      ),
    ])
    return {
      items: parseDockItems(rawItems),
      placement: parseDockPlacement(rawPlacement),
    }
  }

  /** Reads the vault's values once it is open; the previous vault's dock is not shown meanwhile. */
  async function loadAsync(): Promise<void> {
    storedItems.value = null
    placement.value = { ...DEFAULT_DOCK_PLACEMENT }
    deviceUuid = null
    const next = await readAsync()
    storedItems.value = next.items
    placement.value = next.placement
  }

  /** Re-reads after a synced change (FR-040). Skipped while an own write is on its way: that read
   * could still return the value before it, and the write's own change event re-reads anyway. */
  async function refreshAsync(): Promise<void> {
    if (pendingWrites > 0) return
    const next = await readAsync()
    if (pendingWrites > 0) return
    if (JSON.stringify(next.items) !== JSON.stringify(storedItems.value))
      storedItems.value = next.items
    if (JSON.stringify(next.placement) !== JSON.stringify(placement.value))
      placement.value = next.placement
  }

  function enqueue(write: () => Promise<void>): Promise<void> {
    pendingWrites += 1
    const done = writes.then(write).finally(() => {
      pendingWrites -= 1
    })
    writes = done.catch(() => undefined)
    return done
  }

  // ponytail: the whole list is one value, so when two devices reorder at the same time the later
  // write wins completely (spec Assumptions). Upgrade path: one CRDT row per entry.
  /** Shows `next` at once and stores it, unavailable apps included (FR-037). */
  function writeItemsAsync(next: DockItemState[]): Promise<void> {
    const value = serializeDockItems(next)
    storedItems.value = JSON.parse(value) as DockItem[]
    return enqueue(() => setPrefAsync(VAULT, DOCK_ITEMS_KEY, value))
  }

  function isPinned(appId: string): boolean {
    return items.value.some(
      (item) => item.kind === 'app' && item.appId === appId,
    )
  }

  /** Adds the app at the end; an app already in the dock stays where it is. */
  function pinAsync(appId: string): Promise<void> {
    if (isPinned(appId)) return Promise.resolve()
    return writeItemsAsync([
      ...items.value,
      { kind: 'app', appId, available: true },
    ])
  }

  function unpinAsync(appId: string): Promise<void> {
    return writeItemsAsync(
      items.value.filter(
        (item) => !(item.kind === 'app' && item.appId === appId),
      ),
    )
  }

  /** Adds a removed control at the end; one already present stays where it is. */
  function addControlAsync(id: DockControlId): Promise<void> {
    if (items.value.some((item) => item.kind === 'control' && item.id === id))
      return Promise.resolve()
    return writeItemsAsync([
      ...items.value,
      { kind: 'control', id, available: true },
    ])
  }

  /** Removes the item at `index` of `items`; the launcher cannot be removed (FR-006). */
  function removeAsync(index: number): Promise<void> {
    const item = items.value[index]
    if (!item) return Promise.resolve()
    if (item.kind === 'control' && item.id === 'launcher')
      return Promise.reject(new Error('the launcher cannot be removed'))
    return writeItemsAsync(items.value.filter((_, i) => i !== index))
  }

  /** Moves the item at `from` to `to`, both indexes into `items`. */
  function moveAsync(from: number, to: number): Promise<void> {
    const next = [...items.value]
    const [item] = next.splice(from, 1)
    if (!item || from === to) return Promise.resolve()
    next.splice(Math.max(0, Math.min(to, next.length)), 0, item)
    return writeItemsAsync(next)
  }

  function setPlacementAsync(patch: Partial<DockPlacement>): Promise<void> {
    const next = { ...placement.value, ...patch }
    placement.value = next
    return enqueue(async () =>
      setPrefAsync(
        await deviceScopeAsync(),
        DOCK_PLACEMENT_KEY,
        JSON.stringify(next),
      ),
    )
  }

  return {
    items,
    placement: readonly(placement),
    effective,
    isPinned,
    loadAsync,
    refreshAsync,
    pinAsync,
    unpinAsync,
    addControlAsync,
    removeAsync,
    moveAsync,
    setPlacementAsync,
  }
}
