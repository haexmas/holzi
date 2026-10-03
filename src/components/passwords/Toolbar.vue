<script setup lang="ts">
/**
 * The first row of the password manager frame (spec 034, FR-007, FR-039): the sidebar button, the
 * search field bound to `?q=` of the place, the button for a new entry and the settings menu.
 * The search text is the only free text a place carries; it is never a secret (the search looks
 * at title, username, URL and tag names only).
 */
import { Search } from '@lucide/vue'

const props = defineProps<{
  sidebarVisible: boolean
}>()

const emit = defineEmits<{
  toggleSidebar: []
}>()

const { t } = useI18n()
const fieldLabels = useFieldLabels()
const router = useTabRouter()

const query = ref(router.route.query.q ?? '')
// A history step to another place or another search puts its own text into the field.
watch(
  () => router.route.query.q ?? '',
  (value) => {
    if (value !== query.value) query.value = value
  },
)

function onUpdate(value: string | number | null | undefined) {
  query.value = String(value ?? '')
  onInput()
}

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
    <UiInput
      :model-value="query"
      type="search"
      :placeholder="t('passwords.search.placeholder')"
      :aria-label="t('passwords.search.label')"
      :labels="fieldLabels.input.value"
      :prepend-icon="Search"
      clearable
      class="max-w-72 min-w-0 [&_input::-webkit-search-cancel-button]:hidden"
      data-testid="passwords-search"
      @update:model-value="onUpdate"
      @keydown.esc.prevent.stop="clear"
    />
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
