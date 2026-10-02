<script setup lang="ts">
import { Search, X } from '@lucide/vue'

/**
 * The first row of the settings frame (spec 023-settings-app, FR-004, FR-023): only the sidebar
 * button and the search, like the COSMIC and GNOME settings. The search button opens a search
 * field in its place; Escape or the clear button empties it first and closes it when empty. What
 * the query finds, the frame shows in the sidebar.
 */
const props = defineProps<{
  sidebarVisible: boolean
}>()

const emit = defineEmits<{
  toggleSidebar: []
  /** Enter in the search field: open the first hit. */
  submit: []
}>()

const query = defineModel<string>('query', { required: true })
const searchOpen = defineModel<boolean>('searchOpen', { required: true })

const { t } = useI18n()
const fieldLabels = useFieldLabels()
const input = useTemplateRef<{ focus: () => void }>('input')
const searchButton = useTemplateRef<{ $el: HTMLElement }>('searchButton')

const sidebarLabel = computed(() =>
  props.sidebarVisible
    ? t('settings.sidebar.hide')
    : t('settings.sidebar.show'),
)

async function openSearch() {
  searchOpen.value = true
  await nextTick()
  input.value?.focus()
}

async function clearOrClose() {
  if (query.value) {
    query.value = ''
    input.value?.focus()
    return
  }
  searchOpen.value = false
  await nextTick()
  searchButton.value?.$el.focus()
}
</script>

<template>
  <div class="flex items-center gap-1">
    <UiButton
      variant="ghost"
      size="icon"
      class="shrink-0"
      :aria-label="sidebarLabel"
      :tooltip="sidebarLabel"
      :aria-expanded="sidebarVisible"
      aria-controls="settings-sidebar"
      data-testid="settings-sidebar-toggle"
      @click="emit('toggleSidebar')"
    >
      <Icon name="lucide:panel-left" class="size-4" />
    </UiButton>
    <UiInput
      v-if="searchOpen"
      ref="input"
      v-model="query"
      type="search"
      :placeholder="t('settings.search.placeholder')"
      :aria-label="t('settings.search.label')"
      :labels="fieldLabels.input.value"
      :prepend-icon="Search"
      clearable
      class="max-w-72 min-w-0 [&_input::-webkit-search-cancel-button]:hidden"
      data-testid="settings-search"
      @keydown.enter.prevent="emit('submit')"
      @keydown.esc.prevent.stop="clearOrClose"
    >
      <!-- With text the field's own clear button shows; an empty search keeps its close button. -->
      <template #append>
        <UiButton
          v-if="!query"
          :icon="X"
          :tooltip="t('settings.search.close')"
          variant="ghost"
          class="shadow-none"
          data-testid="settings-search-clear"
          @click.prevent="clearOrClose"
        />
      </template>
    </UiInput>
    <UiButton
      v-else
      ref="searchButton"
      variant="ghost"
      size="icon"
      class="shrink-0"
      :aria-label="t('settings.search.open')"
      :tooltip="t('settings.search.open')"
      data-testid="settings-search-open"
      @click="openSearch"
    >
      <Icon name="lucide:search" class="size-4" />
    </UiButton>
  </div>
</template>
