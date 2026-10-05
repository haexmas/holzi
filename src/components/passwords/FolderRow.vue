<script setup lang="ts">
/**
 * A subfolder of the open folder in the list (spec 036, US3, FR-011, FR-018, FR-020): it opens on a
 * click, is selected together with entries, has the context menu and menu button of a folder, is
 * dimmed while it lies cut in the Ablage, and takes a drop of entries and folders, which move into
 * it. Only ids travel.
 */
import type { GroupRow } from '@bindings/GroupRow'
import type { MenuCommand, MenuEntry } from '~/lib/passwords/menus'
import {
  draggedIds,
  FOLDER_MIME,
  ITEMS_MIME,
  itemsPayload,
  parseItemsPayload,
} from '~/lib/passwords/dnd'
import { displayTitle } from '~/lib/passwords/format'

const props = defineProps<{
  group: GroupRow
  /** Entries directly inside. */
  itemCount: number
  selected?: boolean
  selecting?: boolean
  dimmed?: boolean
  tabStop?: boolean
  /** Builds the menu of a row when it opens; the same function for every row. */
  menuFor: (id: string) => readonly MenuEntry[]
}>()

const emit = defineEmits<{
  activate: [event: MouseEvent]
  longPress: []
  menu: [command: MenuCommand]
  menuOpen: []
  focus: []
  /** Entries or folders dropped on the row. */
  drop: [ids: string[]]
}>()

const { t } = useI18n()
const selection = usePasswordsSelectionStore()

const name = computed(
  () => displayTitle(props.group.name) ?? t('passwords.folders.unknown'),
)
const dropping = ref(false)

const button = useTemplateRef<HTMLElement>('button')
const press = usePasswordsRowPress(button, {
  activate: (event) => emit('activate', event),
  longPress: () => emit('longPress'),
})

function onDragStart(event: DragEvent) {
  const ids = draggedIds(props.group.id, selection.ids)
  event.dataTransfer?.setData(ITEMS_MIME, itemsPayload(ids))
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function onDragOver(event: DragEvent) {
  const types = event.dataTransfer?.types ?? []
  if (!types.includes(ITEMS_MIME) && !types.includes(FOLDER_MIME)) return
  event.preventDefault()
  dropping.value = true
}

function onDrop(event: DragEvent) {
  dropping.value = false
  const folder = event.dataTransfer?.getData(FOLDER_MIME)
  const ids = folder
    ? [folder]
    : parseItemsPayload(event.dataTransfer?.getData(ITEMS_MIME))
  if (ids.length === 0) return
  event.preventDefault()
  emit('drop', ids)
}
</script>

<template>
  <PasswordsEntryMenu
    :entries="() => menuFor(group.id)"
    @run="emit('menu', $event)"
    @open="emit('menuOpen')"
  >
    <li
      class="group flex items-center pr-2 hover:bg-foreground/5"
      :class="[
        selected ? 'bg-primary/10' : '',
        dimmed ? 'opacity-50 grayscale' : '',
        dropping ? 'ring-2 ring-primary ring-inset' : '',
      ]"
      @contextmenu.stop
      @dragover="onDragOver"
      @dragleave="dropping = false"
      @drop="onDrop"
    >
      <button
        ref="button"
        type="button"
        class="flex min-h-12 min-w-0 flex-1 items-center gap-4 py-2.5 pl-4 text-left focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:ring-inset"
        :aria-pressed="selecting ? selected : undefined"
        :tabindex="tabStop ? 0 : -1"
        draggable="true"
        :data-row-id="group.id"
        :data-testid="`passwords-list-folder-${group.id}`"
        @click="press.onClick"
        @contextmenu="press.onContextmenu"
        @focus="emit('focus')"
        @dragstart="onDragStart"
        @pointerdown="press.onPointerdown"
      >
        <ShadcnCheckbox
          v-if="selecting"
          :model-value="selected"
          class="pointer-events-none shrink-0"
          tabindex="-1"
          aria-hidden="true"
        />
        <span
          class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-background"
          :style="group.color ? { color: group.color } : undefined"
          aria-hidden="true"
        >
          <span class="size-5">
            <PasswordsEntryIcon :value="group.icon" fallback="lucide:folder" />
          </span>
        </span>
        <span class="min-w-0 flex-1 truncate">{{ name }}</span>
        <span v-if="itemCount" class="text-sm text-muted-foreground">
          {{ itemCount }}
        </span>
        <Icon
          name="lucide:chevron-right"
          class="size-4 shrink-0 text-muted-foreground"
        />
      </button>
      <PasswordsEntryMenuButton
        :entries="() => menuFor(group.id)"
        :label="t('passwords.folders.menu', { name })"
        :data-testid="`passwords-list-folder-menu-${group.id}`"
        @run="emit('menu', $event)"
        @open="emit('menuOpen')"
      />
    </li>
  </PasswordsEntryMenu>
</template>
