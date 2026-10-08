<script setup lang="ts">
/**
 * The open folder as list or grid (spec 044 FR-003, FR-008): both scroll virtually, so a folder
 * with 50 000 entries mounts only the rows on screen. Loading, empty and error states replace the
 * list; a folder the system denies shows why.
 */
import type { Entry } from '@bindings/Entry'
import type { FilesError } from '@bindings/FilesError'
import type { SourceRef } from '@bindings/SourceRef'
import type { FilesView } from '~/composables/useFilesPrefs'
import { formatFileSize } from '~/lib/passwords/format'

const props = defineProps<{
  source: SourceRef
  entries: Entry[]
  loading: boolean
  error: FilesError | null
  view: FilesView
}>()

const emit = defineEmits<{ activate: [entry: Entry] }>()

const { t, d } = useI18n()

const ROW_HEIGHT = 40
const TILE_WIDTH = 132
const TILE_HEIGHT = 140

const container = useTemplateRef<HTMLElement>('container')
const { width } = useElementSize(container)
const columns = computed(() =>
  Math.max(1, Math.floor((width.value || TILE_WIDTH) / TILE_WIDTH)),
)

/** The list's rows: one entry each; the grid's rows: `columns` entries each. */
const rows = computed<Entry[][]>(() => {
  if (props.view === 'list') return props.entries.map((entry) => [entry])
  const out: Entry[][] = []
  for (let i = 0; i < props.entries.length; i += columns.value) {
    out.push(props.entries.slice(i, i + columns.value))
  }
  return out
})

const { list, containerProps, wrapperProps, scrollTo } = useVirtualList(rows, {
  itemHeight: () => (props.view === 'list' ? ROW_HEIGHT : TILE_HEIGHT),
  overscan: 8,
})

// A new folder starts at the top.
watch(
  () => props.entries,
  () => scrollTo(0),
)

function modified(entry: Entry): string {
  return entry.modifiedMs === null
    ? ''
    : d(new Date(entry.modifiedMs), { dateStyle: 'medium', timeStyle: 'short' })
}

function size(entry: Entry): string {
  return entry.kind === 'dir' || entry.size === null
    ? ''
    : formatFileSize(entry.size)
}

const errorText = computed(() => {
  if (!props.error) return ''
  const key = `files.error.${props.error.code}`
  return t(key) === key ? props.error.message : t(key)
})
</script>

<template>
  <div ref="container" class="h-full min-h-0">
    <div
      v-if="error"
      class="flex h-full flex-col items-center justify-center gap-2 p-6 text-center text-sm text-muted-foreground"
      data-testid="files-error"
    >
      <Icon name="lucide:circle-alert" class="size-8" />
      <p>{{ errorText }}</p>
    </div>
    <div
      v-else-if="!loading && entries.length === 0"
      class="flex h-full items-center justify-center p-6 text-sm text-muted-foreground"
      data-testid="files-empty"
    >
      {{ t('files.empty') }}
    </div>
    <div
      v-else
      v-bind="containerProps"
      class="h-full overflow-y-auto"
      role="list"
      :aria-busy="loading"
      data-testid="files-entries"
    >
      <div v-bind="wrapperProps">
        <template v-if="view === 'list'">
          <button
            v-for="{ data: [entry], index } in list"
            :key="entry?.path ?? index"
            type="button"
            role="listitem"
            class="flex w-full items-center gap-3 px-3 text-left text-sm hover:bg-accent focus-visible:bg-accent focus-visible:outline-none"
            :class="entry?.hidden ? 'opacity-60' : ''"
            :style="{ height: `${ROW_HEIGHT}px` }"
            :data-testid="`files-entry-${entry?.name}`"
            @click="entry && emit('activate', entry)"
          >
            <template v-if="entry">
              <span class="size-5 shrink-0">
                <FilesEntryIcon :source="source" :entry="entry" />
              </span>
              <span class="min-w-0 flex-1 truncate">{{ entry.name }}</span>
              <Icon
                v-if="entry.symlink"
                name="lucide:link"
                class="size-3.5 shrink-0 text-muted-foreground"
                :aria-label="t('files.symlink')"
              />
              <Icon
                v-if="entry.holziOwned"
                name="lucide:lock"
                class="size-3.5 shrink-0 text-muted-foreground"
                :aria-label="t('files.holziOwned')"
              />
              <span
                class="hidden w-24 shrink-0 text-right text-muted-foreground @md:block"
              >
                {{ size(entry) }}
              </span>
              <span
                class="hidden w-40 shrink-0 text-right text-muted-foreground @xl:block"
              >
                {{ modified(entry) }}
              </span>
            </template>
          </button>
        </template>
        <template v-else>
          <div
            v-for="{ data: row, index } in list"
            :key="index"
            class="flex"
            :style="{ height: `${TILE_HEIGHT}px` }"
          >
            <button
              v-for="entry in row"
              :key="entry.path"
              type="button"
              role="listitem"
              class="flex flex-col items-center gap-1 rounded p-2 text-xs hover:bg-accent focus-visible:bg-accent focus-visible:outline-none"
              :class="entry.hidden ? 'opacity-60' : ''"
              :style="{ width: `${TILE_WIDTH}px` }"
              :data-testid="`files-entry-${entry.name}`"
              @click="emit('activate', entry)"
            >
              <span class="size-24">
                <FilesEntryIcon :source="source" :entry="entry" thumbnail />
              </span>
              <span class="line-clamp-2 w-full text-center break-all">
                {{ entry.name }}
              </span>
            </button>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
