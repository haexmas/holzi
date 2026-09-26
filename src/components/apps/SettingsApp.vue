<script setup lang="ts">
/**
 * The settings frame (spec 023-settings-app, contracts §2): category sidebar, a header with the
 * location's title, one-line description and — for sub-views — a back arrow, and the location's
 * view below as the only scrolling area. Every view is a tab location (spec 020); the registry in
 * `lib/settings/registry.ts` supplies titles and hierarchy. The frame is a size container, so the
 * sidebar reacts to the window's width, not the screen's (FR-004).
 */
import type { DeviceInfo } from '~/composables/useDevice'
import { SETTINGS_DEVICE_KEY } from '~/components/settings/deviceContext'
import {
  headerBack,
  locationFor,
  parentPathOf,
  SETTINGS_LOCATIONS,
} from '~/lib/settings/registry'

const { t } = useI18n()
const { errString } = useErrorString()
const { currentDeviceInfoAsync } = useDevice()
const router = useTabRouter()
const tab = useWmTab()
const wm = useWindowManagerStore()

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
const description = computed(() =>
  current.value ? t(current.value.location.descriptionKey) : '',
)
const parentPath = computed(() =>
  current.value ? parentPathOf(current.value.location) : undefined,
)
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
  <div class="@container flex h-full min-h-0">
    <SettingsSidebar />
    <main class="flex min-w-0 flex-1 flex-col">
      <header class="flex items-start gap-2 border-b border-border px-6 py-4">
        <UiButton
          v-if="parentPath"
          variant="ghost"
          size="icon"
          class="-ml-2 shrink-0"
          :aria-label="backLabel"
          :tooltip="backLabel"
          data-testid="settings-back"
          @click="goBack"
        >
          <Icon name="lucide:arrow-left" class="size-4" />
        </UiButton>
        <div class="min-w-0">
          <h1 class="truncate text-2xl font-semibold">{{ title }}</h1>
          <p class="text-sm text-muted-foreground">{{ description }}</p>
        </div>
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-6">
        <p v-if="loadError" class="text-sm text-destructive" role="alert">
          {{ t('errors.deviceInfoFailed') }}: {{ loadError }}
        </p>
        <div v-else-if="deviceInfo" class="max-w-2xl">
          <WmRouterView />
        </div>
      </div>
    </main>
  </div>
</template>
