/**
 * The open folder of a file browser tab (spec 044 FR-003, FR-006): its entries, loading and error,
 * kept current by watching it (device) and by loading again when the window gets the focus back,
 * in case the watch missed a change.
 */
import type { Entry } from '@bindings/Entry'
import type { FilesError } from '@bindings/FilesError'
import type { SourceRef } from '@bindings/SourceRef'
import { asFilesError } from '~/composables/useFiles'

export function useFilesFolder(
  source: Ref<SourceRef>,
  path: Ref<string | null>,
) {
  const { listAsync, watchAsync, unwatchAsync } = useFiles()
  const entries = shallowRef<Entry[]>([])
  const loading = ref(false)
  const error = ref<FilesError | null>(null)
  let generation = 0
  let watchId: number | null = null
  let reloadTimer: ReturnType<typeof setTimeout> | null = null

  async function loadAsync() {
    const current = path.value
    if (current === null) return
    const mine = ++generation
    loading.value = true
    try {
      const listed = await listAsync(source.value, current)
      if (mine !== generation) return
      entries.value = listed
      error.value = null
    } catch (caught) {
      if (mine !== generation) return
      entries.value = []
      error.value = asFilesError(caught) ?? {
        code: 'notFound',
        message: String(caught),
      }
    } finally {
      if (mine === generation) loading.value = false
    }
  }

  function reloadSoon() {
    if (reloadTimer) clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => {
      reloadTimer = null
      void loadAsync()
    }, 150)
  }

  async function stopWatching() {
    const id = watchId
    watchId = null
    if (id !== null) await unwatchAsync(id).catch(() => {})
  }

  async function startWatching() {
    await stopWatching()
    const current = path.value
    if (current === null || source.value.kind !== 'device') return
    try {
      const id = await watchAsync(current, reloadSoon)
      // The folder changed while the watch was starting: keep only the newest.
      if (path.value === current) watchId = id
      else await unwatchAsync(id).catch(() => {})
    } catch {
      // No watch here (a folder the system cannot watch): focus reloads still keep it current.
    }
  }

  function onFocus() {
    if (document.visibilityState !== 'hidden') void loadAsync()
  }

  watch(
    [source, path],
    () => {
      void loadAsync()
      void startWatching()
    },
    { immediate: true },
  )
  onMounted(() => {
    window.addEventListener('focus', onFocus)
    document.addEventListener('visibilitychange', onFocus)
  })
  onBeforeUnmount(() => {
    window.removeEventListener('focus', onFocus)
    document.removeEventListener('visibilitychange', onFocus)
    if (reloadTimer) clearTimeout(reloadTimer)
    generation++
    void stopWatching()
  })

  return { entries, loading, error, reload: loadAsync }
}
