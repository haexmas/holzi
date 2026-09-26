<script setup lang="ts">
/**
 * The settings frame (spec 023-settings-app, contracts §2): category sidebar, a header with the
 * location's title behind a back arrow (sub-views) or the category's icon in the same slot
 * (category start pages, so the title does not shift), and the location's view below as the only
 * scrolling area. Header and view share one centered column, so the list does not stretch across
 * a wide window. Every view is a tab location (spec 020); the registry in
 * `lib/settings/registry.ts` supplies titles and hierarchy.
 *
 * The frame is a size container, so the sidebar follows the window's width, not the screen's
 * (FR-004): from `@2xl` it sits beside the content and can be hidden, below it is gone and opens
 * over the whole frame from the header. Both states are local to the open tab, nothing is kept.
 */
import type { DeviceInfo } from '~/composables/useDevice'
import { SETTINGS_DEVICE_KEY } from '~/components/settings/deviceContext'
import {
  categoryOf,
  headerBack,
  locationFor,
  parentPathOf,
  SETTINGS_CATEGORIES,
  SETTINGS_LOCATIONS,
} from '~/lib/settings/registry'

const { t } = useI18n()
const { errString } = useErrorString()
const { currentDeviceInfoAsync } = useDevice()
const router = useTabRouter()
const tab = useWmTab()
const wm = useWindowManagerStore()

const root = useTemplateRef<HTMLElement>('root')
const sidebar = useTemplateRef<{
  $el: HTMLElement
  focusSearch(): void
  focusActive(): void
}>('sidebar')
/** Wide window: the operator hid the sidebar. */
const wideHidden = ref(false)
/** Narrow window: the sidebar is open over the content. */
const menuOpen = ref(false)

/** Read from the sidebar's CSS rather than a second copy of the `@2xl` threshold. */
function isWide(): boolean {
  const el = sidebar.value?.$el
  return el ? getComputedStyle(el).position !== 'absolute' : true
}

const { width } = useElementSize(root)
watch(width, () => {
  if (menuOpen.value && isWide()) menuOpen.value = false
})

const sidebarClass = computed(() => [
  'absolute inset-y-0 left-0 z-20 w-full overflow-hidden transition-[translate,width,visibility] duration-200 ease-out motion-reduce:transition-none',
  '@2xl:static @2xl:z-auto @2xl:translate-x-0 @2xl:border-r @2xl:border-sidebar-border',
  menuOpen.value ? 'visible translate-x-0' : 'invisible -translate-x-full',
  wideHidden.value
    ? '@2xl:invisible @2xl:w-0 @2xl:border-r-0'
    : '@2xl:visible @2xl:w-64',
])

async function openSidebar(focus: 'list' | 'search') {
  if (isWide()) wideHidden.value = false
  else menuOpen.value = true
  await nextTick()
  if (focus === 'search') sidebar.value?.focusSearch()
  else sidebar.value?.focusActive()
}

async function closeSidebar() {
  if (isWide()) wideHidden.value = true
  else menuOpen.value = false
  await nextTick()
  root.value
    ?.querySelector<HTMLElement>('[data-testid="settings-sidebar-show"]')
    ?.focus()
}

function onSidebarEscape() {
  if (menuOpen.value && !isWide()) void closeSidebar()
}

const deviceInfo = ref<DeviceInfo | null>(null)
const loadError = ref<string | null>(null)

async function reloadDeviceInfoAsync() {
  loadError.value = null
  try {
    deviceInfo.value = await currentDeviceInfoAsync()
  } catch (error) {
    loadError.value = errString(error)
  }
}

provide(SETTINGS_DEVICE_KEY, {
  info: deviceInfo as Ref<DeviceInfo>,
  reloadAsync: reloadDeviceInfoAsync,
})

const current = computed(() => locationFor(router.route.path))
const title = computed(() =>
  current.value ? t(current.value.location.titleKey, current.value.params) : '',
)
const parentPath = computed(() =>
  current.value ? parentPathOf(current.value.location) : undefined,
)
const categoryIcon = computed(() => {
  const id = categoryOf(router.route.path)
  return SETTINGS_CATEGORIES.find((category) => category.id === id)?.icon
})
const backLabel = computed(() => {
  const parent = SETTINGS_LOCATIONS.find(
    (location) => location.id === current.value?.location.parent,
  )
  return parent ? t('settings.back', { title: t(parent.titleKey) }) : ''
})

function goBack() {
  const history = tab.tabId ? wm.historyOf(tab.tabId) : undefined
  if (!parentPath.value || !history) return
  const step = headerBack(history, parentPath.value)
  if (step.kind === 'back') router.back()
  else router.push(step.path)
}

onMounted(reloadDeviceInfoAsync)
</script>

<template>
  <div
    ref="root"
    class="@container relative flex h-full min-h-0 overflow-hidden"
  >
    <SettingsSidebar
      id="settings-sidebar"
      ref="sidebar"
      :class="sidebarClass"
      @navigate="menuOpen = false"
      @hide="closeSidebar"
      @close="onSidebarEscape"
    />
    <main class="flex min-w-0 flex-1 flex-col" :inert="menuOpen || undefined">
      <header
        class="flex h-15 shrink-0 items-center gap-1 border-b border-border pr-6 pl-3"
      >
        <div
          class="shrink-0 items-center gap-1"
          :class="wideHidden ? 'flex' : 'flex @2xl:hidden'"
        >
          <UiButton
            variant="ghost"
            size="icon"
            :aria-label="t('settings.sidebar.show')"
            :tooltip="t('settings.sidebar.show')"
            aria-expanded="false"
            aria-controls="settings-sidebar"
            data-testid="settings-sidebar-show"
            @click="openSidebar('list')"
          >
            <Icon name="lucide:panel-left" class="size-4" />
          </UiButton>
          <UiButton
            variant="ghost"
            size="icon"
            :aria-label="t('settings.search.open')"
            :tooltip="t('settings.search.open')"
            data-testid="settings-search-open"
            @click="openSidebar('search')"
          >
            <Icon name="lucide:search" class="size-4" />
          </UiButton>
        </div>
        <div
          class="mx-auto flex w-full max-w-3xl min-w-0 items-center gap-2 pl-1"
        >
          <UiButton
            v-if="parentPath"
            variant="ghost"
            size="icon"
            class="shrink-0"
            :aria-label="backLabel"
            :tooltip="backLabel"
            data-testid="settings-back"
            @click="goBack"
          >
            <Icon name="lucide:arrow-left" class="size-4" />
          </UiButton>
          <span
            v-else-if="categoryIcon"
            class="flex size-9 shrink-0 items-center justify-center text-muted-foreground"
            aria-hidden="true"
            data-testid="settings-category-icon"
          >
            <Icon :name="categoryIcon" class="size-4" />
          </span>
          <h1 class="min-w-0 truncate text-xl font-semibold">{{ title }}</h1>
        </div>
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-4">
        <div class="mx-auto w-full max-w-3xl">
          <p v-if="loadError" class="text-sm text-destructive" role="alert">
            {{ t('errors.deviceInfoFailed') }}: {{ loadError }}
          </p>
          <WmRouterView v-else-if="deviceInfo" />
        </div>
      </div>
    </main>
  </div>
</template>
