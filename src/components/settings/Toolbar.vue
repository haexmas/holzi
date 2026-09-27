<script setup lang="ts">
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
const input = useTemplateRef<HTMLInputElement>('input')
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
    <label v-if="searchOpen" class="relative w-full max-w-72 min-w-0">
      <span class="sr-only">{{ t('settings.search.label') }}</span>
      <Icon
        name="lucide:search"
        class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
      />
      <input
        ref="input"
        v-model="query"
        type="search"
        :placeholder="t('settings.search.placeholder')"
        class="h-9 w-full rounded-full border border-input bg-background pr-9 pl-9 text-sm [&::-webkit-search-cancel-button]:hidden focus:ring-2 focus:ring-ring focus:outline-none"
        data-testid="settings-search"
        @keydown.enter.prevent="emit('submit')"
        @keydown.esc.prevent.stop="clearOrClose"
      />
      <button
        type="button"
        class="absolute top-1/2 right-1.5 flex size-6 -translate-y-1/2 items-center justify-center rounded-full text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        :aria-label="
          query ? t('settings.search.clear') : t('settings.search.close')
        "
        data-testid="settings-search-clear"
        @click="clearOrClose"
      >
        <Icon name="lucide:x" class="size-4" />
      </button>
    </label>
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
