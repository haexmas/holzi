<script setup lang="ts">
/**
 * Thread list, sidebar chrome and the rename/delete UI — extracted from
 * `src/pages/chat/[instance].vue` (spec 015-workspace-shell, T010, plan
 * research R10). Presentational only: all state and the actions it emits
 * live in `useThreadSidebar` (page-level), except the edited-row input's
 * focus/select, which this component owns since it is DOM-local to a row
 * this component renders.
 */
import { nextTick, ref, watch } from 'vue'
import type { Thread } from '~/composables/useChat'

const props = defineProps<{
  instanceName: string
  busy: boolean
  threads: Thread[]
  activeThreadId: string | null
  editingThreadId: string | null
  draftTitle: string
  editTitleError: string | null
  renamingThreadId: string | null
  deleteCandidate: Thread | null
  deleteError: string | null
  deletingThread: boolean
  historyDurationLabel: (createdAt: number) => string
  openingTimeLabel: (createdAt: number) => string
}>()

const emit = defineEmits<{
  'update:draftTitle': [value: string]
  newChat: []
  selectThread: [id: string]
  startEditing: [thread: Thread]
  saveTitle: []
  cancelEditing: []
  requestDelete: [thread: Thread]
  closeDeleteDialog: [open: boolean]
  confirmDelete: []
  lock: []
  openSettings: []
}>()

const { t } = useI18n()

const editingTitleInput = ref<HTMLInputElement | null>(null)
watch(
  () => props.editingThreadId,
  (id) => {
    if (id === null) return
    void nextTick(() => {
      editingTitleInput.value?.focus()
      editingTitleInput.value?.select()
    })
  },
)
</script>

<template>
  <aside
    class="hidden md:flex w-64 shrink-0 border-r border-border bg-background p-4 flex-col gap-4 overflow-y-auto"
  >
    <div class="flex items-center gap-3 min-w-0">
      <div
        class="h-9 w-9 shrink-0 rounded-xl bg-foreground text-background flex items-center justify-center"
      >
        <Icon name="lucide:sparkles" class="h-4 w-4" />
      </div>
      <div class="min-w-0">
        <div class="text-sm font-semibold">Holzi</div>
        <div
          class="text-xs text-muted-foreground truncate"
          :title="instanceName"
        >
          {{ instanceName }}
        </div>
      </div>
    </div>

    <UiButton
      class="w-full justify-start gap-2"
      variant="outline"
      :disabled="busy"
      @click="emit('newChat')"
    >
      <Icon name="lucide:plus" class="h-4 w-4" />
      {{ t('chat.newChat') }}
    </UiButton>

    <div class="flex items-center justify-between px-1">
      <span class="text-xs font-medium text-muted-foreground">{{
        t('chat.threads.title')
      }}</span>
      <span class="text-[10px] text-muted-foreground">{{
        threads.length
      }}</span>
    </div>
    <div class="space-y-1">
      <div
        v-for="thread in threads"
        :key="thread.id"
        class="group flex w-full min-w-0 items-center gap-1 rounded-lg text-sm transition-colors hover:bg-accent focus-within:bg-accent"
        :class="{ 'bg-accent font-medium': activeThreadId === thread.id }"
      >
        <template v-if="editingThreadId === thread.id">
          <div class="min-w-0 flex-1 px-2 py-1.5">
            <input
              ref="editingTitleInput"
              :value="draftTitle"
              class="w-full rounded border border-border bg-background px-2 py-1 text-sm outline-none focus:border-foreground/50"
              :aria-label="t('chat.threads.editTitle')"
              :disabled="renamingThreadId === thread.id"
              @input="
                emit(
                  'update:draftTitle',
                  ($event.target as HTMLInputElement).value,
                )
              "
              @keydown.enter.prevent="emit('saveTitle')"
              @keydown.esc.prevent="emit('cancelEditing')"
            />
            <p
              v-if="editTitleError"
              class="mt-1 text-xs text-destructive"
              role="alert"
            >
              {{ editTitleError }}
            </p>
          </div>
          <button
            type="button"
            class="shrink-0 rounded p-1.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            :aria-label="t('chat.threads.saveTitle')"
            :title="t('chat.threads.saveTitle')"
            :disabled="renamingThreadId === thread.id"
            @click.stop="emit('saveTitle')"
          >
            <Icon name="lucide:check" class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="mr-1 shrink-0 rounded p-1.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            :aria-label="t('chat.threads.cancelEdit')"
            :title="t('chat.threads.cancelEdit')"
            :disabled="renamingThreadId === thread.id"
            @click.stop="emit('cancelEditing')"
          >
            <Icon name="lucide:x" class="h-4 w-4" />
          </button>
        </template>
        <template v-else>
          <button
            type="button"
            class="min-w-0 flex-1 truncate px-2 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
            @click="emit('selectThread', thread.id)"
          >
            <span class="truncate">{{ thread.title }}</span>
          </button>
          <div
            class="flex max-w-0 shrink-0 items-center overflow-hidden opacity-0 transition-[max-width,opacity] duration-150 group-hover:max-w-14 group-hover:opacity-100 group-focus-within:max-w-14 group-focus-within:opacity-100"
          >
            <button
              type="button"
              class="rounded p-1.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('chat.threads.editTitle')"
              :title="t('chat.threads.editTitle')"
              @click.stop="emit('startEditing', thread)"
            >
              <Icon name="lucide:pencil" class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="mr-1 rounded p-1.5 text-muted-foreground hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('chat.threads.deleteTitle')"
              :title="t('chat.threads.deleteTitle')"
              @click.stop="emit('requestDelete', thread)"
            >
              <Icon name="lucide:trash-2" class="h-3.5 w-3.5" />
            </button>
          </div>
          <span
            class="w-8 shrink-0 rounded pr-1 text-right text-xs font-normal tabular-nums text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            tabindex="0"
            :title="openingTimeLabel(thread.createdAt)"
            :aria-label="`${t('chat.threads.duration', { duration: historyDurationLabel(thread.createdAt) })}, ${openingTimeLabel(thread.createdAt)}`"
          >
            {{ historyDurationLabel(thread.createdAt) }}
          </span>
        </template>
      </div>
      <div
        v-if="threads.length === 0"
        class="px-3 py-2 text-xs text-muted-foreground"
      >
        {{ t('chat.threads.empty') }}
      </div>
    </div>

    <div class="flex-1" />
    <button
      type="button"
      class="flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
      @click="emit('openSettings')"
    >
      <Icon name="lucide:settings-2" class="h-4 w-4" />
      {{ t('chat.settings') }}
    </button>
    <UiButton
      class="justify-start gap-2"
      size="sm"
      variant="ghost"
      data-testid="lock-instance-sidebar"
      @click="emit('lock')"
    >
      <Icon name="lucide:lock-keyhole" class="h-4 w-4" />
      {{ t('chat.lock') }}
    </UiButton>
  </aside>

  <UiDrawerModal
    v-if="deleteCandidate"
    :open="deleteCandidate !== null"
    :title="t('chat.threads.deleteDialogTitle')"
    @update:open="emit('closeDeleteDialog', $event)"
  >
    <template #content>
      <div class="space-y-3 px-6 py-2">
        <p class="text-sm">
          {{
            t('chat.threads.deleteConfirm', { title: deleteCandidate.title })
          }}
        </p>
        <p v-if="deleteError" class="text-sm text-destructive" role="alert">
          {{ deleteError }}
        </p>
      </div>
    </template>
    <template #footer>
      <div class="flex justify-end gap-2">
        <UiButton
          type="button"
          variant="outline"
          :disabled="deletingThread"
          @click="emit('closeDeleteDialog', false)"
        >
          {{ t('chat.cancel') }}
        </UiButton>
        <UiButton
          type="button"
          variant="destructive"
          :loading="deletingThread"
          @click="emit('confirmDelete')"
        >
          {{ t('chat.threads.deleteConfirmButton') }}
        </UiButton>
      </div>
    </template>
  </UiDrawerModal>
</template>
