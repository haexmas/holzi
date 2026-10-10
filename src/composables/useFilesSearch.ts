/**
 * The search of a file browser tab (spec 044 FR-027, FR-028, FR-029): it runs while the tab's
 * location holds a query, from the open folder down. A new query, filter or folder cancels the
 * running search first, so only hits of the current one show; hits arrive in batches and are kept
 * sorted by score.
 */
import type { SearchHit } from '@bindings/SearchHit'
import type { SourceRef } from '@bindings/SourceRef'
import { asFilesError } from '~/composables/useFiles'
import { type FilesFilter, searchFilters } from '~/lib/files/filters'

export function useFilesSearch(
  source: Ref<SourceRef>,
  path: Ref<string | null>,
  query: Ref<string>,
  filter: Ref<FilesFilter>,
  showHidden: Ref<boolean>,
) {
  const { searchStartAsync, searchCancelAsync } = useFiles()
  const hits = shallowRef<SearchHit[]>([])
  const running = ref(false)
  const truncated = ref(false)
  /** Folders of a storage searched so far (a storage shows its progress, FR-030). */
  const dirs = ref(0)
  const error = ref<string | null>(null)
  let generation = 0
  let current: Promise<string | null> | null = null

  function stop() {
    generation++
    const previous = current
    current = null
    running.value = false
    if (previous)
      void previous.then((id) => {
        if (id) void searchCancelAsync(id).catch(() => {})
      })
  }

  function start() {
    stop()
    hits.value = []
    truncated.value = false
    dirs.value = 0
    error.value = null
    const text = query.value.trim()
    const folder = path.value
    if (!text || folder === null) return
    const mine = generation
    running.value = true
    current = searchStartAsync(
      source.value,
      folder,
      text,
      searchFilters(filter.value, Date.now()),
      showHidden.value,
      (event) => {
        if (mine !== generation) return
        if (event.kind === 'hits') {
          hits.value = [...hits.value, ...event.hits].sort(
            (a, b) => b.score - a.score,
          )
        } else if (event.kind === 'progress') {
          dirs.value = event.dirs
        } else {
          truncated.value = event.truncated
          running.value = false
          current = null
        }
      },
    ).catch((caught: unknown) => {
      if (mine === generation) {
        running.value = false
        const code = asFilesError(caught)?.code
        error.value = code ?? String(caught)
      }
      return null
    })
  }

  // A key, not the objects: the filter is parsed anew on every change of the tab's location (an
  // opened file, too), and only a real change may start a new search.
  watch(
    () =>
      JSON.stringify([
        source.value,
        path.value,
        query.value.trim(),
        filter.value,
        showHidden.value,
      ]),
    start,
    { immediate: true },
  )
  onBeforeUnmount(stop)

  return { hits, running, truncated, dirs, error }
}
