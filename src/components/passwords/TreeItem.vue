<script setup lang="ts">
/**
 * One folder in the sidebar tree and, recursively, its subfolders (spec 034, US2, FR-009): expand
 * and collapse, the entry count, a menu with the actions that also work without a mouse (new
 * subfolder, edit, up, down) and drag and drop: a folder dropped on a sibling goes before it, entries
 * dropped on a folder move into it. The row only reports what happened; the sidebar acts.
 */
import type { GroupRow } from '@bindings/GroupRow'
import { FOLDER_MIME, ITEMS_MIME, parseItemsPayload } from '~/lib/passwords/dnd'
import type { TreeNode } from '~/lib/passwords/tree'

const props = defineProps<{
  node: TreeNode<GroupRow>
  depth: number
  /** The ids of the folders on this level in display order. */
  siblingIds: string[]
  activeId: string | null
  /** Offers "Delete" (it moves the folder to the trash). */
  canDelete?: boolean
}>()

const expanded = defineModel<Set<string>>('expanded', { required: true })

const emit = defineEmits<{
  select: [groupId: string]
  edit: [group: GroupRow]
  newChild: [parentId: string]
  move: [groupId: string, direction: 'up' | 'down']
  dropFolder: [dragged: string, target: string]
  dropItems: [groupId: string, itemIds: string[]]
  remove: [group: GroupRow]
}>()

const { t } = useI18n()

const group = computed(() => props.node.group)
const isOpen = computed(() => expanded.value.has(group.value.id))
const hasChildren = computed(() => props.node.children.length > 0)
const position = computed(() => props.siblingIds.indexOf(group.value.id))
const dropping = ref(false)

function toggleOpen() {
  const next = new Set(expanded.value)
  if (next.has(group.value.id)) next.delete(group.value.id)
  else next.add(group.value.id)
  expanded.value = next
}

function onDragStart(event: DragEvent) {
  event.dataTransfer?.setData(FOLDER_MIME, group.value.id)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function accepts(event: DragEvent): boolean {
  const types = event.dataTransfer?.types ?? []
  return types.includes(ITEMS_MIME) || types.includes(FOLDER_MIME)
}

function onDragOver(event: DragEvent) {
  if (!accepts(event)) return
  event.preventDefault()
  dropping.value = true
}

function onDrop(event: DragEvent) {
  dropping.value = false
  const folder = event.dataTransfer?.getData(FOLDER_MIME)
  const items = event.dataTransfer?.getData(ITEMS_MIME)
  if (folder) {
    event.preventDefault()
    emit('dropFolder', folder, group.value.id)
  } else if (items) {
    const ids = parseItemsPayload(items)
    if (ids.length === 0) return
    event.preventDefault()
    emit('dropItems', group.value.id, ids)
  }
}
</script>

<template>
  <li :data-testid="`passwords-folder-${group.id}`">
    <div
      class="group flex min-h-9 items-center gap-1 rounded-lg pr-1 hover:bg-foreground/5"
      :class="[
        activeId === group.id ? 'bg-foreground/10 font-medium' : '',
        dropping ? 'ring-2 ring-primary' : '',
      ]"
      :style="{ paddingLeft: `${depth * 12 + 4}px` }"
      draggable="true"
      @dragstart="onDragStart"
      @dragover="onDragOver"
      @dragleave="dropping = false"
      @drop="onDrop"
    >
      <button
        type="button"
        class="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground"
        :class="hasChildren ? '' : 'invisible'"
        :aria-label="
          isOpen
            ? t('passwords.folders.collapse')
            : t('passwords.folders.expand')
        "
        :aria-expanded="isOpen"
        @click="toggleOpen"
      >
        <Icon
          :name="isOpen ? 'lucide:chevron-down' : 'lucide:chevron-right'"
          class="size-4"
        />
      </button>
      <button
        type="button"
        class="flex min-w-0 flex-1 items-center gap-2 py-1.5 text-left"
        @click="emit('select', group.id)"
      >
        <span
          class="size-4 shrink-0"
          :style="group.color ? { color: group.color } : undefined"
        >
          <PasswordsEntryIcon :value="group.icon" fallback="lucide:folder" />
        </span>
        <span class="min-w-0 flex-1 truncate">{{ group.name }}</span>
        <span v-if="node.itemCount" class="text-xs text-muted-foreground">{{
          node.itemCount
        }}</span>
      </button>
      <ShadcnDropdownMenu>
        <ShadcnDropdownMenuTrigger as-child>
          <button
            type="button"
            class="flex size-7 shrink-0 items-center justify-center rounded text-muted-foreground opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus:opacity-100 [@media(hover:none)]:opacity-100"
            :aria-label="
              t('passwords.folders.menu', { name: group.name ?? '' })
            "
            :data-testid="`passwords-folder-menu-${group.id}`"
          >
            <Icon name="lucide:ellipsis" class="size-4" />
          </button>
        </ShadcnDropdownMenuTrigger>
        <ShadcnDropdownMenuContent align="end">
          <ShadcnDropdownMenuItem @select="emit('newChild', group.id)">
            {{ t('passwords.folders.newChild') }}
          </ShadcnDropdownMenuItem>
          <ShadcnDropdownMenuItem @select="emit('edit', group)">
            {{ t('passwords.folders.edit') }}
          </ShadcnDropdownMenuItem>
          <ShadcnDropdownMenuItem
            :disabled="position <= 0"
            @select="emit('move', group.id, 'up')"
          >
            {{ t('passwords.folders.moveUp') }}
          </ShadcnDropdownMenuItem>
          <ShadcnDropdownMenuItem
            :disabled="position === -1 || position >= siblingIds.length - 1"
            @select="emit('move', group.id, 'down')"
          >
            {{ t('passwords.folders.moveDown') }}
          </ShadcnDropdownMenuItem>
          <ShadcnDropdownMenuItem
            v-if="canDelete"
            @select="emit('remove', group)"
          >
            {{ t('passwords.folders.delete') }}
          </ShadcnDropdownMenuItem>
        </ShadcnDropdownMenuContent>
      </ShadcnDropdownMenu>
    </div>
    <ul v-if="hasChildren && isOpen" class="flex flex-col">
      <PasswordsTreeItem
        v-for="child in node.children"
        :key="child.group.id"
        v-model:expanded="expanded"
        :node="child"
        :depth="depth + 1"
        :sibling-ids="node.children.map((c) => c.group.id)"
        :active-id="activeId"
        :can-delete="canDelete"
        @select="emit('select', $event)"
        @edit="emit('edit', $event)"
        @new-child="emit('newChild', $event)"
        @move="(id, direction) => emit('move', id, direction)"
        @drop-folder="(dragged, target) => emit('dropFolder', dragged, target)"
        @drop-items="(id, ids) => emit('dropItems', id, ids)"
        @remove="emit('remove', $event)"
      />
    </ul>
  </li>
</template>
