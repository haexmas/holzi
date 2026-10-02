<script setup lang="ts">
/**
 * The sidebar of the password manager (spec 034, US2, FR-009, FR-011): all entries, the folder
 * tree with expand and collapse, the tags with their counts, the trash and the actions for folders
 * and tags. Folders reorder by drag and drop and by the menu actions "up" and "down" (also with
 * keyboard and touch); entries dragged from the list move into a folder. Only ids travel.
 */
import { toast } from 'vue-sonner'
import type { GroupRow } from '@bindings/GroupRow'
import type { Target } from '@bindings/Target'
import { ITEMS_MIME, parseItemsPayload } from '~/lib/passwords/dnd'
import {
  buildTree,
  moveBefore,
  moveDown,
  moveUp,
  type TreeNode,
} from '~/lib/passwords/tree'

const emit = defineEmits<{
  /** The user picked a place: the frame closes the overlay of a narrow window. */
  navigated: []
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const router = useTabRouter()
const store = usePasswordsStore()
const { reorderGroupsAsync, moveAsync } = usePasswords()

const tree = computed(() => buildTree(store.groups, store.headers))
const expanded = ref(new Set<string>())
const folderDialog = ref(false)
const editing = ref<GroupRow | null>(null)
const parentForNew = ref<string | null>(null)
const tagManager = ref(false)
const rootDropping = ref(false)
const deleteOpen = ref(false)
const deleteTargets = ref<Target[]>([])

const place = computed(() => {
  const path = router.route.path
  if (path === '/trash') return { kind: 'trash' as const }
  if (path.startsWith('/folder/'))
    return { kind: 'folder' as const, id: router.route.params.id ?? '' }
  if (path === '/') return { kind: 'all' as const }
  return { kind: 'other' as const }
})
const activeFolderId = computed(() =>
  place.value.kind === 'folder' ? place.value.id : null,
)
const activeTag = computed(() => router.route.query.tag ?? null)

function go(path: string) {
  router.push(path)
  emit('navigated')
}

function newFolder(parentId: string | null) {
  editing.value = null
  parentForNew.value = parentId
  folderDialog.value = true
  if (parentId) expanded.value = new Set(expanded.value).add(parentId)
}

function askDeleteFolder(group: GroupRow) {
  deleteTargets.value = [{ kind: 'group', id: group.id }]
  deleteOpen.value = true
}

/** After a folder went to the trash the window leaves it if it was showing it. */
function afterFolderDelete() {
  if (
    activeFolderId.value &&
    deleteTargets.value.some((t) => t.id === activeFolderId.value)
  ) {
    router.replace('/')
  }
}

function editFolder(group: GroupRow) {
  editing.value = group
  folderDialog.value = true
}

function siblingsOf(parentId: string | null): string[] {
  const level =
    parentId === null
      ? tree.value.roots
      : (findNode(tree.value.roots, parentId)?.children ?? [])
  return level.map((node) => node.group.id)
}

function findNode(
  nodes: TreeNode<GroupRow>[],
  id: string,
): TreeNode<GroupRow> | undefined {
  for (const node of nodes) {
    if (node.group.id === id) return node
    const inner = findNode(node.children, id)
    if (inner) return inner
  }
  return undefined
}

function parentOf(id: string): string | null {
  return store.groups.find((group) => group.id === id)?.parentId ?? null
}

async function reorderAsync(parentId: string | null, ordered: string[]) {
  try {
    await reorderGroupsAsync(parentId, ordered)
    await store.quietReloadAsync()
  } catch (cause) {
    toast.error(errString(cause))
  }
}

async function moveFolderAsync(id: string, direction: 'up' | 'down') {
  const parentId = parentOf(id)
  const ids = siblingsOf(parentId)
  const next = direction === 'up' ? moveUp(ids, id) : moveDown(ids, id)
  await reorderAsync(parentId, next)
}

async function dropFolderAsync(dragged: string, target: string) {
  const parentId = parentOf(target)
  // Dropping on a folder of another level does nothing: nesting is done by moving entries.
  if (parentOf(dragged) !== parentId) return
  await reorderAsync(
    parentId,
    moveBefore(siblingsOf(parentId), dragged, target),
  )
}

async function dropItemsAsync(groupId: string | null, ids: string[]) {
  try {
    const result = await moveAsync(
      ids.map((id) => ({ kind: 'item' as const, id })),
      groupId,
    )
    toast.success(t('passwords.moved', { count: result.moved }, result.moved))
    await store.quietReloadAsync()
  } catch (cause) {
    toast.error(errString(cause))
  }
}

function onRootDragOver(event: DragEvent) {
  if (!event.dataTransfer?.types.includes(ITEMS_MIME)) return
  event.preventDefault()
  rootDropping.value = true
}

async function onRootDrop(event: DragEvent) {
  rootDropping.value = false
  const ids = parseItemsPayload(event.dataTransfer?.getData(ITEMS_MIME))
  if (ids.length === 0) return
  event.preventDefault()
  await dropItemsAsync(null, ids)
}
</script>

<template>
  <div
    class="flex h-full min-h-0 flex-col gap-3 overflow-y-auto p-2"
    data-testid="passwords-sidebar-content"
  >
    <ul class="flex flex-col gap-0.5">
      <li>
        <button
          type="button"
          class="flex min-h-9 w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left hover:bg-foreground/5"
          :class="[
            place.kind === 'all' && !activeTag
              ? 'bg-foreground/10 font-medium'
              : '',
            rootDropping ? 'ring-2 ring-primary' : '',
          ]"
          data-testid="passwords-all"
          @click="go('/')"
          @dragover="onRootDragOver"
          @dragleave="rootDropping = false"
          @drop="onRootDrop"
        >
          <Icon name="lucide:key-round" class="size-4 shrink-0" />
          <span class="min-w-0 flex-1 truncate">{{
            t('passwords.sidebar.all')
          }}</span>
          <span class="text-xs text-muted-foreground">{{
            store.headers.length - tree.trash.itemCount
          }}</span>
        </button>
      </li>
      <PasswordsTreeItem
        v-for="node in tree.roots"
        :key="node.group.id"
        v-model:expanded="expanded"
        :node="node"
        :depth="0"
        :sibling-ids="tree.roots.map((root) => root.group.id)"
        :active-id="activeFolderId"
        can-delete
        @select="go(`/folder/${$event}`)"
        @edit="editFolder"
        @new-child="newFolder"
        @move="moveFolderAsync"
        @drop-folder="dropFolderAsync"
        @drop-items="dropItemsAsync"
        @remove="askDeleteFolder"
      />
    </ul>

    <section v-if="store.displayTags.length" class="flex flex-col gap-1">
      <h2 class="px-3 text-xs font-semibold text-muted-foreground uppercase">
        {{ t('passwords.fields.tags') }}
      </h2>
      <ul class="flex flex-col gap-0.5">
        <li v-for="tag in store.displayTags" :key="tag.id">
          <button
            type="button"
            class="flex min-h-9 w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left hover:bg-foreground/5"
            :class="
              activeTag !== null && tag.ids.includes(activeTag)
                ? 'bg-foreground/10 font-medium'
                : ''
            "
            :data-testid="`passwords-tag-filter-${tag.id}`"
            @click="go(`/?tag=${tag.id}`)"
          >
            <Icon
              name="lucide:tag"
              class="size-4 shrink-0"
              :style="tag.color ? { color: tag.color } : undefined"
            />
            <span class="min-w-0 flex-1 truncate">{{ tag.name }}</span>
            <span class="text-xs text-muted-foreground">{{
              tag.itemCount
            }}</span>
          </button>
        </li>
      </ul>
    </section>

    <div class="mt-auto flex flex-col gap-1">
      <button
        type="button"
        class="flex min-h-9 w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left hover:bg-foreground/5"
        :class="place.kind === 'trash' ? 'bg-foreground/10 font-medium' : ''"
        data-testid="passwords-trash"
        @click="go('/trash')"
      >
        <Icon name="lucide:trash-2" class="size-4 shrink-0" />
        <span class="min-w-0 flex-1 truncate">{{
          t('passwords.trash.title')
        }}</span>
        <span
          v-if="tree.trash.itemCount"
          class="text-xs text-muted-foreground"
          >{{ tree.trash.itemCount }}</span
        >
      </button>
      <UiButton
        variant="ghost"
        size="sm"
        data-testid="passwords-open-generator"
        @click="go('/generator')"
      >
        <Icon name="lucide:dices" class="size-4" />
        {{ t('passwords.generator.title') }}
      </UiButton>
      <UiButton
        variant="ghost"
        size="sm"
        data-testid="passwords-open-import"
        @click="go('/import')"
      >
        <Icon name="lucide:file-down" class="size-4" />
        {{ t('passwords.import.open') }}
      </UiButton>
      <UiButton
        variant="outline"
        size="sm"
        data-testid="passwords-new-folder"
        @click="newFolder(null)"
      >
        <Icon name="lucide:folder-plus" class="size-4" />
        {{ t('passwords.folders.new') }}
      </UiButton>
      <UiButton
        variant="ghost"
        size="sm"
        data-testid="passwords-manage-tags"
        @click="tagManager = true"
      >
        <Icon name="lucide:tags" class="size-4" />
        {{ t('passwords.tags.manage') }}
      </UiButton>
    </div>

    <PasswordsFolderDialog
      v-model:open="folderDialog"
      :group="editing"
      :parent-id="parentForNew"
    />
    <PasswordsTagManager v-model:open="tagManager" />
    <PasswordsDeleteDialog
      v-model:open="deleteOpen"
      :targets="deleteTargets"
      @done="afterFolderDelete"
    />
  </div>
</template>
