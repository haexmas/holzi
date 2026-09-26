<script setup lang="ts">
/**
 * Category sidebar of the settings (spec 023-settings-app, FR-001, FR-004, FR-010, FR-023). On top
 * the button that hides it and the search field; below either the categories in registry order,
 * the current one highlighted, or the search hits. A click always pushes the target's own
 * location, so a category entered from the sidebar starts at its overview. Where and how wide the
 * sidebar is, the frame (`apps/SettingsApp.vue`) decides; the content keeps its width while the
 * frame slides it, so nothing reflows during the transition.
 */
import { categoryOf, SETTINGS_CATEGORIES } from '~/lib/settings/registry'
import { searchSettings } from '~/lib/settings/search'

const emit = defineEmits<{
  /** After a category or search hit was chosen. */
  navigate: []
  /** The hide button. */
  hide: []
  /** Escape with an empty search field. */
  close: []
}>()

const { t } = useI18n()
const router = useTabRouter()

const active = computed(() => categoryOf(router.route.path))
const query = ref('')
const searching = computed(() => query.value.trim().length > 0)
const hits = computed(() => searchSettings(query.value, (key) => t(key)))

const searchInput = useTemplateRef<HTMLInputElement>('searchInput')
const list = useTemplateRef<HTMLElement>('list')

function iconOf(categoryId: string, icon?: string): string {
  return (
    icon ??
    SETTINGS_CATEGORIES.find((category) => category.id === categoryId)?.icon ??
    'lucide:settings-2'
  )
}

function go(path: string) {
  router.push(path)
  query.value = ''
  emit('navigate')
}

function onSearchEnter() {
  const first = hits.value[0]
  if (first) go(first.path)
}

function onEscape() {
  if (searching.value) query.value = ''
  else emit('close')
}

defineExpose({
  focusSearch() {
    searchInput.value?.focus()
  },
  focusActive() {
    list.value
      ?.querySelector<HTMLElement>('[aria-current="page"], button')
      ?.focus()
  },
})
</script>

<template>
  <nav
    class="bg-sidebar text-sidebar-foreground"
    :aria-label="t('wm.apps.settings')"
    @keydown.esc="onEscape"
  >
    <div class="flex h-full min-w-64 flex-col">
      <div class="flex h-15 shrink-0 items-center gap-2 px-3">
        <UiButton
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('settings.sidebar.hide')"
          :tooltip="t('settings.sidebar.hide')"
          aria-expanded="true"
          aria-controls="settings-sidebar"
          data-testid="settings-sidebar-hide"
          @click="emit('hide')"
        >
          <Icon name="lucide:panel-left" class="size-4" />
        </UiButton>
        <label class="relative min-w-0 flex-1">
          <span class="sr-only">{{ t('settings.search.label') }}</span>
          <Icon
            name="lucide:search"
            class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <input
            ref="searchInput"
            v-model="query"
            type="search"
            :placeholder="t('settings.search.placeholder')"
            class="h-9 w-full rounded-md border border-input bg-background pr-2 pl-8 text-sm focus:ring-2 focus:ring-ring focus:outline-none"
            data-testid="settings-search"
            @keydown.enter.prevent="onSearchEnter"
          />
        </label>
      </div>

      <div ref="list" class="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        <template v-if="searching">
          <ul
            v-if="hits.length > 0"
            class="flex flex-col gap-1"
            :aria-label="t('settings.search.label')"
          >
            <li v-for="hit in hits" :key="`${hit.location.id}:${hit.label}`">
              <button
                type="button"
                class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm hover:bg-sidebar-accent"
                data-testid="settings-search-hit"
                @click="go(hit.path)"
              >
                <Icon
                  :name="iconOf(hit.location.category, hit.location.icon)"
                  class="size-5 shrink-0"
                />
                <span class="flex min-w-0 flex-col">
                  <span class="truncate">{{ hit.label }}</span>
                  <span
                    v-if="hit.trail.length > 0"
                    class="truncate text-xs text-muted-foreground"
                  >
                    {{ hit.trail.join(' › ') }}
                  </span>
                </span>
              </button>
            </li>
          </ul>
          <p
            v-else
            class="px-3 py-2 text-sm text-muted-foreground"
            role="status"
          >
            {{ t('settings.search.noResults') }}
          </p>
        </template>

        <ul v-else class="flex flex-col gap-1">
          <li v-for="category in SETTINGS_CATEGORIES" :key="category.id">
            <button
              type="button"
              class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm whitespace-nowrap hover:bg-sidebar-accent"
              :class="
                active === category.id
                  ? 'bg-sidebar-accent font-medium text-primary'
                  : ''
              "
              :aria-current="active === category.id ? 'page' : undefined"
              :data-testid="`settings-category-${category.id}`"
              @click="go(category.path)"
            >
              <Icon :name="category.icon" class="size-5 shrink-0" />
              <span class="truncate">{{ t(category.titleKey) }}</span>
            </button>
          </li>
        </ul>
      </div>
    </div>
  </nav>
</template>
