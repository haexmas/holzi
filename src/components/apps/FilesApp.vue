<script lang="ts">
import type { Entry } from '@bindings/Entry'
import { filesLocation, parseFilesPlace } from '~/lib/files/registry'
import { childPath, parentPath, visibleEntries } from '~/lib/files/state'

/** How many file browser frames are open, across all windows of the process. */
let openFrames = 0
</script>

<script setup lang="ts">
/**
 * The file browser frame (spec 044, US1), laid out like the password manager: a toolbar, below it
 * the sidebar (known places, drives) and the open folder as list or grid, with the viewer over it.
 * Where the tab stands is its location (`lib/files/registry.ts`), so moving between folders keeps
 * this component mounted and session restore (spec 022) brings tab and open file back.
 */

const { t } = useI18n()
const router = useTabRouter()
const prefs = useFilesPrefs()
const { sourcesAsync, releaseTabAsync } = useFiles()

const place = computed(() => parseFilesPlace(router.route))
const source = computed(
  () => place.value?.source ?? { kind: 'device' as const },
)
const path = computed(() => place.value?.path ?? null)

const sources = useAsyncState(() => sourcesAsync(), null)

// The start (no path yet) opens the home folder, or the first drive where there is none.
watchEffect(() => {
  if (path.value !== null) return
  const data = sources.state.value
  if (!data) return
  const home = data.known.find((known) => known.name === 'home')?.path
  const start = home ?? data.drives[0]?.path
  if (start)
    router.replace(filesLocation({ source: source.value, path: start }))
})

const folder = useFilesFolder(source, path)
const shown = computed(() =>
  visibleEntries(
    folder.entries.value,
    prefs.sort.value,
    prefs.showHidden.value,
  ),
)

onMounted(() => {
  openFrames += 1
  void prefs.ensureLoaded()
})
// FR-016: the media server URLs of this tab end with it.
const { tabId } = useWmTab()

onBeforeUnmount(() => {
  void releaseTabAsync(tabId).catch(() => {})
  openFrames -= 1
  if (openFrames > 0) return
  clearFilesThumbnails()
})

function go(target: string) {
  router.push(filesLocation({ source: source.value, path: target }))
}

function goUp() {
  const parent = path.value ? parentPath(path.value) : null
  if (parent) go(parent)
}

function activate(entry: Entry) {
  if (entry.kind === 'dir') {
    if (!entry.noAccess) go(entry.path)
    return
  }
  router.setQuery({ open: entry.name }, { push: true })
}

/** The file in the viewer: its entry once the folder is listed, until then only name and path (a
 * restored tab opens the file before its folder arrives). */
const openEntry = computed(
  (): (Pick<Entry, 'name' | 'path'> & Partial<Entry>) | null => {
    const name = place.value?.open
    if (!name || !path.value) return null
    return (
      folder.entries.value.find((entry) => entry.name === name) ?? {
        name,
        path: childPath(path.value, name),
      }
    )
  },
)

function closeViewer() {
  router.setQuery({ open: null })
}

function showSibling(entry: Entry) {
  router.setQuery({ open: entry.name })
}

const root = useTemplateRef<HTMLElement>('root')
const sidebar = useTemplateRef<HTMLElement>('sidebar')
const { wideHidden, menuOpen, visible, toggle, close } = useSidebarFrame(
  root,
  () => sidebar.value,
)

const sidebarClass = computed(() => [
  'absolute inset-0 z-20 bg-background transition-[translate,width,visibility] duration-200 ease-out motion-reduce:transition-none',
  '@2xl:static @2xl:z-auto @2xl:shrink-0 @2xl:overflow-hidden @2xl:translate-x-0',
  menuOpen.value ? 'visible translate-x-0' : 'invisible -translate-x-full',
  wideHidden.value ? '@2xl:invisible @2xl:w-0' : '@2xl:visible @2xl:w-60',
])

function onSidebarPick(target: string) {
  close()
  go(target)
}
</script>

<template>
  <div
    ref="root"
    class="@container flex h-full min-h-0 flex-col bg-muted/20"
    data-files-app
  >
    <FilesToolbar
      class="h-13 shrink-0 px-2"
      :path="path"
      :sidebar-visible="visible"
      :can-go-up="!!path && parentPath(path) !== null"
      @toggle-sidebar="toggle"
      @go="go"
      @up="goUp"
      @reload="folder.reload()"
    />
    <div class="relative flex min-h-0 flex-1 overflow-hidden">
      <nav
        id="files-sidebar"
        ref="sidebar"
        :class="sidebarClass"
        :aria-label="t('files.sidebar.label')"
        data-testid="files-sidebar"
      >
        <FilesSidebar
          :sources="sources.state.value"
          :current="path"
          @pick="onSidebarPick"
        />
      </nav>
      <main class="relative min-w-0 flex-1" data-testid="files-main">
        <FilesFolderView
          :source="source"
          :entries="shown"
          :loading="folder.loading.value"
          :error="folder.error.value"
          :view="prefs.view.value"
          @activate="activate"
        />
        <FilesViewer
          v-if="openEntry"
          :source="source"
          :entry="openEntry"
          :siblings="shown"
          @close="closeViewer"
          @show="showSibling"
        />
      </main>
    </div>
  </div>
</template>
