<script setup lang="ts">
/**
 * The first row of the password manager frame (spec 034, FR-007, FR-039): the sidebar button, the
 * search field bound to `?q=` of the place, the button for a new entry and the settings menu.
 * The search text is the only free text a place carries; it is never a secret (the search looks
 * at title, username, URL and tag names only).
 */
const props = defineProps<{
  sidebarVisible: boolean
}>()

const emit = defineEmits<{
  toggleSidebar: []
}>()

const { t } = useI18n()
const router = useTabRouter()

const query = ref(router.route.query.q ?? '')
// A history step to another place or another search puts its own text into the field.
watch(
  () => router.route.query.q ?? '',
  (value) => {
    if (value !== query.value) query.value = value
  },
)

function onInput() {
  router.setQuery({ q: query.value.trim() ? query.value : null })
}

function clear() {
  query.value = ''
  onInput()
}

const sidebarLabel = computed(() =>
  props.sidebarVisible
    ? t('passwords.sidebar.hide')
    : t('passwords.sidebar.show'),
)

const settingsOpen = ref(false)
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
      aria-controls="passwords-sidebar"
      data-testid="passwords-sidebar-toggle"
      @click="emit('toggleSidebar')"
    >
      <Icon name="lucide:panel-left" class="size-4" />
    </UiButton>
    <label class="relative w-full max-w-72 min-w-0">
      <span class="sr-only">{{ t('passwords.search.label') }}</span>
      <Icon
        name="lucide:search"
        class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
      />
      <input
        v-model="query"
        type="search"
        :placeholder="t('passwords.search.placeholder')"
        class="h-9 w-full rounded-full border border-input bg-background pr-9 pl-9 text-sm [&::-webkit-search-cancel-button]:hidden focus:ring-2 focus:ring-ring focus:outline-none"
        data-testid="passwords-search"
        @input="onInput"
        @keydown.esc.prevent.stop="clear"
      />
      <button
        v-if="query"
        type="button"
        class="absolute top-1/2 right-1.5 flex size-6 -translate-y-1/2 items-center justify-center rounded-full text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        :aria-label="t('passwords.search.clear')"
        data-testid="passwords-search-clear"
        @click="clear"
      >
        <Icon name="lucide:x" class="size-4" />
      </button>
    </label>
    <div class="ml-auto flex items-center gap-1">
      <UiButton
        variant="ghost"
        size="icon"
        class="shrink-0"
        :aria-label="t('passwords.settings.open')"
        :tooltip="t('passwords.settings.open')"
        data-testid="passwords-settings"
        @click="settingsOpen = true"
      >
        <Icon name="lucide:settings-2" class="size-4" />
      </UiButton>
      <UiButton
        class="shrink-0"
        :aria-label="t('passwords.new')"
        data-testid="passwords-new"
        @click="router.push('/entry/new')"
      >
        <Icon name="lucide:plus" class="size-4" />
        <span class="hidden @md:inline">{{ t('passwords.new') }}</span>
      </UiButton>
    </div>
    <PasswordsClipboardSetting v-model:open="settingsOpen" />
  </div>
</template>
