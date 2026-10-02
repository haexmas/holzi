<script setup lang="ts">
/**
 * The bar above the list while entries are selected (spec 034, US2, FR-012): move to a folder, add
 * or remove a tag, and delete (which moves to the trash). Every mass action names the number of
 * entries it touches and asks before it runs.
 */
import { toast } from 'vue-sonner'
import { buildTree, flattenFolders } from '~/lib/passwords/tree'

const emit = defineEmits<{
  delete: []
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const store = usePasswordsStore()
const selection = usePasswordsSelectionStore()
const { moveAsync, setTagsAsync } = usePasswords()

type Pending =
  | { kind: 'move'; groupId: string | null }
  | { kind: 'addTag'; name: string }
  | { kind: 'removeTag'; name: string }

const pending = ref<Pending | null>(null)
const busy = ref(false)

const folders = computed(() =>
  flattenFolders(buildTree(store.groups, store.headers).roots),
)

/** The tags that at least one selected entry carries, for "remove tag". */
const selectedTags = computed(() => {
  const ids = new Set(selection.ids)
  const names = new Map<string, string>()
  for (const header of store.headers) {
    if (!ids.has(header.id)) continue
    for (const tag of header.tags) names.set(tag.id, tag.name)
  }
  return [...names.values()].sort((a, b) => a.localeCompare(b))
})

function ask(next: Pending) {
  pending.value = next
}

async function confirmAsync() {
  const action = pending.value
  if (!action) return
  busy.value = true
  const ids = [...selection.ids]
  try {
    if (action.kind === 'move') {
      await moveAsync(
        ids.map((id) => ({ kind: 'item' as const, id })),
        action.groupId,
      )
    } else if (action.kind === 'addTag') {
      if (!action.name.trim()) return
      await setTagsAsync(ids, [action.name.trim()], [])
    } else {
      if (!action.name) return
      await setTagsAsync(ids, [], [action.name])
    }
    await store.quietReloadAsync()
    pending.value = null
    selection.clear()
  } catch (cause) {
    toast.error(errString(cause))
  } finally {
    busy.value = false
  }
}

const targetValue = computed({
  get: () =>
    pending.value?.kind === 'move' ? (pending.value.groupId ?? '') : '',
  set: (value: string) => {
    if (pending.value?.kind === 'move') pending.value.groupId = value || null
  },
})
</script>

<template>
  <div
    v-if="selection.active"
    class="flex flex-wrap items-center gap-2 rounded-xl bg-muted px-3 py-2"
    role="toolbar"
    :aria-label="t('passwords.selection.label')"
    data-testid="passwords-selection"
  >
    <span
      class="mr-auto text-sm font-medium"
      data-testid="passwords-selection-count"
    >
      {{
        t(
          'passwords.selection.count',
          { count: selection.count },
          selection.count,
        )
      }}
    </span>
    <UiButton
      variant="outline"
      size="sm"
      data-testid="passwords-selection-move"
      @click="ask({ kind: 'move', groupId: null })"
    >
      <Icon name="lucide:folder-input" class="size-4" />
      {{ t('passwords.selection.move') }}
    </UiButton>
    <UiButton
      variant="outline"
      size="sm"
      data-testid="passwords-selection-tag"
      @click="ask({ kind: 'addTag', name: '' })"
    >
      <Icon name="lucide:tag" class="size-4" />
      {{ t('passwords.selection.addTag') }}
    </UiButton>
    <UiButton
      v-if="selectedTags.length"
      variant="outline"
      size="sm"
      data-testid="passwords-selection-untag"
      @click="ask({ kind: 'removeTag', name: selectedTags[0] ?? '' })"
    >
      <Icon name="lucide:tag" class="size-4" />
      {{ t('passwords.selection.removeTag') }}
    </UiButton>
    <UiButton
      variant="outline"
      size="sm"
      data-testid="passwords-selection-delete"
      @click="emit('delete')"
    >
      <Icon name="lucide:trash-2" class="size-4" />
      {{ t('passwords.selection.delete') }}
    </UiButton>
    <UiButton
      variant="ghost"
      size="sm"
      data-testid="passwords-selection-clear"
      @click="selection.clear()"
    >
      {{ t('passwords.selection.clear') }}
    </UiButton>
  </div>

  <ShadcnAlertDialog
    :open="pending !== null"
    @update:open="(value: boolean) => !value && (pending = null)"
  >
    <ShadcnAlertDialogContent v-if="pending">
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>
          {{
            t(
              `passwords.selection.confirm.${pending.kind}`,
              { count: selection.count },
              selection.count,
            )
          }}
        </ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription as-child>
          <div class="flex flex-col gap-2">
            <select
              v-if="pending.kind === 'move'"
              v-model="targetValue"
              class="h-9 rounded-md border border-input bg-background px-2 text-sm"
              :aria-label="t('passwords.selection.target')"
              data-testid="passwords-selection-target"
            >
              <option value="">{{ t('passwords.selection.topLevel') }}</option>
              <option
                v-for="folder in folders"
                :key="folder.id"
                :value="folder.id"
              >
                {{ '– '.repeat(folder.depth) }}{{ folder.name }}
              </option>
            </select>
            <ShadcnInput
              v-else-if="pending.kind === 'addTag'"
              v-model="pending.name"
              :placeholder="t('passwords.editor.tagPlaceholder')"
              data-testid="passwords-selection-tagname"
            />
            <select
              v-else
              v-model="pending.name"
              class="h-9 rounded-md border border-input bg-background px-2 text-sm"
              :aria-label="t('passwords.fields.tags')"
            >
              <option v-for="name in selectedTags" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </div>
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel>{{
          t('passwords.cancel')
        }}</ShadcnAlertDialogCancel>
        <UiButton
          :loading="busy"
          data-testid="passwords-selection-confirm"
          @click="confirmAsync"
        >
          {{ t('passwords.selection.run') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
