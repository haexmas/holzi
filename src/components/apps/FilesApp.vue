<script lang="ts">
import { toast } from 'vue-sonner'
import type { Entry } from '@bindings/Entry'
import type { SourceRef } from '@bindings/SourceRef'
import type { TransferOp } from '@bindings/TransferOp'
import { asFilesError } from '~/composables/useFiles'
import {
  clickEntry,
  type ClickKeys,
  type DragPayload,
  dropOp,
  EMPTY_SELECTION,
  type FilesClipboard,
  menuSelection,
  pasteRefusal,
  pruneSelection,
  type Selection,
} from '~/lib/files/clipboard'
import { entriesMenu, type FilesCommand, folderMenu } from '~/lib/files/menus'
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
 * Managing files (US3): selection, the context menu, holzi's clipboard, dragging inside holzi and
 * from the system, and the transfers they start (`stores/filesTransfers.ts`).
 */

const { t } = useI18n()
const router = useTabRouter()
const prefs = useFilesPrefs()
const { sourcesAsync, releaseTabAsync, statAsync, openSystemAsync } = useFiles()
const transfers = useFilesTransfersStore()
const { isAndroid } = useDeviceCapabilities()

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

// --- Managing files (US3) ---

const selection = ref<Selection>(EMPTY_SELECTION)
const selecting = computed(() => selection.value.paths.length > 0)
watch(path, () => (selection.value = EMPTY_SELECTION))
watch(shown, (entries) => {
  selection.value = pruneSelection(
    selection.value,
    entries.map((entry) => entry.path),
  )
})

/** The open folder is one of holzi's own places (FR-037): nothing changes in it. */
const folderOwned = ref(false)
watch(
  [source, path],
  async ([current, folderPath]) => {
    folderOwned.value = false
    if (folderPath === null) return
    const entry = await statAsync(current, folderPath).catch(() => null)
    if (path.value === folderPath)
      folderOwned.value = entry?.holziOwned ?? false
  },
  { immediate: true },
)

const entryAt = (target: string) =>
  shown.value.find((entry) => entry.path === target)

function onPress(entry: Entry, keys: ClickKeys) {
  const result = clickEntry(
    selection.value,
    entry.path,
    shown.value.map((candidate) => candidate.path),
    keys,
  )
  selection.value = result.selection
  if (result.activate) activate(entry)
}

/** What the open context menu acts on: entries, or the folder's empty area (`null`). */
const menuTargets = ref<Selection | null>(null)

function onContext(entry: Entry | null) {
  menuTargets.value = entry ? menuSelection(selection.value, entry.path) : null
}

const canPaste = computed(() => transfers.clipboard !== null && !!path.value)

function entriesOf(targets: Selection): Entry[] {
  return targets.paths.flatMap((target) => entryAt(target) ?? [])
}

const menuEntries = computed(() => {
  const targets = menuTargets.value
  if (!targets)
    return folderMenu({ readOnly: folderOwned.value, canPaste: canPaste.value })
  const entries = entriesOf(targets)
  return entriesMenu({
    count: entries.length,
    singleFile: entries.length === 1 && entries[0]?.kind === 'file',
    readOnly: folderOwned.value || entries.some((entry) => entry.holziOwned),
    selecting: selecting.value,
  })
})

const dimmed = computed(() =>
  transfers.clipboard?.op === 'cut'
    ? transfers.clipboard.items.map((item) => item.path)
    : [],
)

function errorText(cause: unknown): string {
  const code = asFilesError(cause)?.code
  return code ? t(`files.error.${code}`) : String(cause)
}

/** Starts a transfer; a refusal shows as a message. Whether it started. */
async function startAsync(
  op: TransferOp,
  from: SourceRef,
  paths: string[],
  to: string | null,
): Promise<boolean> {
  try {
    await transfers.startAsync(
      op,
      from,
      paths,
      to === null ? null : { source: source.value, path: to },
      tabId,
    )
    return true
  } catch (cause) {
    toast.error(errorText(cause))
    return false
  }
}

/**
 * Copies or moves `clip` into the folder `target` (paste, or a drop inside holzi). Whether the
 * transfer started.
 */
async function placeAsync(
  clip: FilesClipboard,
  target: string,
  targetOwned: boolean,
): Promise<boolean> {
  const refusal = pasteRefusal(clip.op, clip.folder, clip.items, {
    path: target,
    holziOwned: targetOwned,
  })
  if (refusal === 'nothing') return false
  if (refusal) {
    toast.error(t(`files.error.${refusal}`))
    return false
  }
  return await startAsync(
    clip.op === 'cut' ? 'move' : 'copy',
    clip.source as SourceRef,
    clip.items.map((item) => item.path),
    target,
  )
}

const naming = ref(false)
const renaming = ref<Entry | null>(null)
const deleting = ref(false)
const pendingDelete = ref<string[]>([])

function remember(op: FilesClipboard['op'], entries: Entry[]) {
  if (!path.value || !entries.length) return
  transfers.clipboard = {
    op,
    source: source.value,
    folder: path.value,
    items: entries.map((entry) => ({ path: entry.path, kind: entry.kind })),
  }
  selection.value = EMPTY_SELECTION
}

function deleteNow() {
  const paths = pendingDelete.value
  pendingDelete.value = []
  selection.value = EMPTY_SELECTION
  void startAsync('delete', source.value, paths, null)
}

/** Runs a command on `targets` (the context menu's, or the selection for the action bar). */
async function run(command: FilesCommand, targets: Selection | null) {
  const entries = targets ? entriesOf(targets) : []
  const [first] = entries
  switch (command) {
    case 'open':
      if (first) activate(first)
      break
    case 'openSystem':
      if (first)
        await openSystemAsync(source.value, first.path).catch((cause) =>
          toast.error(errorText(cause)),
        )
      break
    case 'select':
      selection.value = {
        paths: [
          ...new Set([...selection.value.paths, ...(targets?.paths ?? [])]),
        ],
        anchor: targets?.anchor ?? null,
      }
      break
    case 'copy':
    case 'cut':
      remember(command, entries)
      break
    case 'paste': {
      const clip = transfers.clipboard
      if (!clip || !path.value) break
      // A refused cut stays on the clipboard, so it can go elsewhere.
      const started = await placeAsync(clip, path.value, folderOwned.value)
      if (started && clip.op === 'cut') transfers.clipboard = null
      break
    }
    case 'rename':
      if (first) {
        renaming.value = first
        naming.value = true
      }
      break
    case 'newFolder':
      renaming.value = null
      naming.value = true
      break
    case 'delete':
      pendingDelete.value = entries.map((entry) => entry.path)
      // Desktops move to the trash; Android deletes for good and asks first (FR-023).
      if (isAndroid.value) deleting.value = true
      else deleteNow()
      break
  }
}

function runMenu(command: FilesCommand) {
  void run(command, menuTargets.value)
}

function runBar(command: FilesCommand) {
  void run(command, selection.value)
}

function onDrop(
  payload: DragPayload,
  target: string,
  keys: { ctrl: boolean; alt: boolean },
) {
  // Dropped onto itself.
  if (payload.items.some((item) => item.path === target)) return
  const sameSource =
    JSON.stringify(payload.source) === JSON.stringify(source.value)
  const op = dropOp(sameSource, {
    ...keys,
    mac: /Mac/.test(navigator.userAgent),
  })
  const owned =
    target === path.value
      ? folderOwned.value
      : (entryAt(target)?.holziOwned ?? false)
  selection.value = EMPTY_SELECTION
  void placeAsync({ ...payload, op }, target, owned)
}

async function importDropped(paths: string[]) {
  if (!path.value) return
  try {
    await transfers.importAsync(
      paths,
      { source: source.value, path: path.value },
      tabId,
    )
  } catch (cause) {
    toast.error(errorText(cause))
  }
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
      <main
        class="relative flex min-w-0 flex-1 flex-col"
        data-testid="files-main"
        @keydown.esc="selection = EMPTY_SELECTION"
      >
        <FilesActionBar
          v-if="selecting || canPaste"
          :selected="selection.paths.length"
          :can-paste="canPaste"
          :read-only="folderOwned"
          @run="runBar"
          @clear="selection = EMPTY_SELECTION"
        />
        <FilesDropTarget
          class="flex-1"
          :disabled="folderOwned || !path"
          @drop="importDropped"
        >
          <FilesMenu :entries="menuEntries" @run="runMenu">
            <FilesFolderView
              :source="source"
              :entries="shown"
              :loading="folder.loading.value"
              :error="folder.error.value"
              :view="prefs.view.value"
              :folder="path ?? ''"
              :selected="selection.paths"
              :dimmed="dimmed"
              @press="onPress"
              @context="onContext"
              @drop="onDrop"
            />
          </FilesMenu>
        </FilesDropTarget>
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
    <FilesTransferBar :tab-id="tabId" />
    <FilesNameDialog
      v-model:open="naming"
      :source="source"
      :folder="path ?? ''"
      :entry="renaming"
    />
    <FilesDeleteDialog
      v-model:open="deleting"
      :count="pendingDelete.length"
      @confirm="deleteNow"
    />
  </div>
</template>
