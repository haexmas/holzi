/**
 * The file browser's device preferences (spec 044 FR-003, data-model.md): list or grid, the sort
 * and whether hidden entries show. Stored per device (ADR 0001), never synced.
 */
import type { PrefScope } from '~/composables/usePreferences'
import {
  DEFAULT_SORT,
  formatSort,
  parseSort,
  type Sort,
} from '~/lib/files/state'

export type FilesView = 'list' | 'grid'

const KEYS = {
  view: 'files.view',
  sort: 'files.sort',
  hidden: 'files.hidden',
} as const

const view = ref<FilesView>('list')
const sort = ref<Sort>(DEFAULT_SORT)
const showHidden = ref(false)
let loaded: Promise<void> | null = null

export function useFilesPrefs() {
  const { getPrefAsync, setPrefAsync } = usePreferences()
  const { currentDeviceInfoAsync } = useDevice()

  async function scopeAsync(): Promise<PrefScope> {
    const device = await currentDeviceInfoAsync()
    return { kind: 'device', uuid: device.vaultDeviceUuid }
  }

  async function loadAsync(): Promise<void> {
    const scope = await scopeAsync()
    const [storedView, storedSort, storedHidden] = await Promise.all([
      getPrefAsync(scope, KEYS.view),
      getPrefAsync(scope, KEYS.sort),
      getPrefAsync(scope, KEYS.hidden),
    ])
    view.value = storedView === 'grid' ? 'grid' : 'list'
    sort.value = parseSort(storedSort)
    showHidden.value = storedHidden === 'true'
  }

  /** Loads once per app process; later calls share the first load. */
  function ensureLoaded(): Promise<void> {
    loaded ??= loadAsync().catch(() => {
      loaded = null
    })
    return loaded
  }

  async function saveAsync(key: string, value: string): Promise<void> {
    await setPrefAsync(await scopeAsync(), key, value)
  }

  function setView(next: FilesView) {
    view.value = next
    void saveAsync(KEYS.view, next)
  }

  function setSort(next: Sort) {
    sort.value = next
    void saveAsync(KEYS.sort, formatSort(next))
  }

  function setShowHidden(next: boolean) {
    showHidden.value = next
    void saveAsync(KEYS.hidden, String(next))
  }

  return {
    view: readonly(view),
    sort: readonly(sort),
    showHidden: readonly(showHidden),
    ensureLoaded,
    setView,
    setSort,
    setShowHidden,
  }
}
