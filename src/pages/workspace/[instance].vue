<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  useChat,
  type ModelLoadErrorEvent,
  type ModelLoadProgressEvent,
  type ModelLoadStatusPayload,
} from '~/composables/useChat'

// Minimal workspace-landing stub for spec 002. Deliberately barebones
// in this feature — the full workspace build-out (widgets, panels) is
// a separate feature. Provides the instance heading + persistent FAB
// that routes to the existing chat page, plus a settings icon linking
// to the settings-screen for this device (spec 002 US3 + US4).
definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const { t } = useI18n()
const chat = useChat()

const preloadStatus = ref<ModelLoadStatusPayload | null>(null)
const preloadError = ref(false)
const unlisteners: UnlistenFn[] = []
let unmounted = false

const instanceName = computed(() => {
  const raw = route.params.instance
  return typeof raw === 'string'
    ? raw
    : Array.isArray(raw)
      ? (raw[0] ?? '')
      : ''
})

const settingsTarget = computed(
  () => `/settings/${encodeURIComponent(instanceName.value)}`,
)

function updatePreloadStatus(status: ModelLoadStatusPayload) {
  preloadStatus.value = status
  preloadError.value = status.status === 'error'
}

function onLoadProgress(event: ModelLoadProgressEvent) {
  if (event.phase === 'ready') {
    void chat
      .modelLoadStatusAsync()
      .then((status) => {
        if (status) updatePreloadStatus(status)
      })
      .catch(() => undefined)
    return
  }
  updatePreloadStatus({
    status: 'loading',
    vaultGeneration: event.vaultGeneration,
    loadId: event.loadId,
    modelId: event.modelId,
    modelName: event.modelName,
    phase: event.phase,
    ...(event.providerName ? { providerName: event.providerName } : {}),
  })
}

function onLoadError(_event: ModelLoadErrorEvent) {
  preloadError.value = true
  void chat
    .modelLoadStatusAsync()
    .then((status) => {
      if (status) updatePreloadStatus(status)
    })
    .catch(() => undefined)
}

function onLoadStatus(status: ModelLoadStatusPayload) {
  updatePreloadStatus(status)
}

onMounted(async () => {
  const subscriptions = await Promise.all([
    chat.onModelLoadProgress(onLoadProgress),
    chat.onModelLoadStatus(onLoadStatus),
    chat.onModelLoadError(onLoadError),
  ])
  for (const unlisten of subscriptions) {
    if (unmounted) unlisten()
    else unlisteners.push(unlisten)
  }
  if (!unmounted) {
    try {
      const status = await chat.modelLoadStatusAsync()
      if (status) updatePreloadStatus(status)
    } catch {
      // Workspace remains usable even when the status snapshot is unavailable.
    }
  }
})

onBeforeUnmount(() => {
  unmounted = true
  for (const unlisten of unlisteners.splice(0)) unlisten()
})
</script>

<template>
  <main class="min-h-screen flex flex-col p-6">
    <header class="flex items-center justify-between gap-3">
      <h1 class="text-2xl font-semibold">
        {{ t('workspace.heading', { instance: instanceName }) }}
      </h1>
      <NuxtLink
        :to="settingsTarget"
        class="p-2 rounded hover:bg-neutral-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
        :title="t('workspace.settings.iconTitle')"
        :aria-label="t('workspace.settings.iconTitle')"
      >
        <Icon name="lucide:settings" class="h-5 w-5" />
      </NuxtLink>
    </header>

    <p
      v-if="preloadStatus?.status === 'loading'"
      class="mt-4 flex items-center gap-2 text-xs text-muted-foreground"
      role="status"
    >
      <Icon
        name="lucide:loader-circle"
        class="h-3.5 w-3.5 animate-spin"
        aria-hidden="true"
      />
      {{
        t('workspace.modelPreload.loading', {
          modelName: preloadStatus.modelName,
        })
      }}
    </p>
    <p
      v-else-if="preloadStatus?.status === 'ready'"
      class="mt-4 flex items-center gap-2 text-xs text-muted-foreground"
      role="status"
    >
      <Icon
        name="lucide:check-circle-2"
        class="h-3.5 w-3.5 text-emerald-600"
        aria-hidden="true"
      />
      {{
        t('workspace.modelPreload.ready', {
          modelName: preloadStatus.modelName,
        })
      }}
    </p>
    <p
      v-else-if="preloadError"
      class="mt-4 flex items-center gap-2 text-xs text-muted-foreground"
      role="status"
    >
      <Icon
        name="lucide:circle-alert"
        class="h-3.5 w-3.5 text-amber-600"
        aria-hidden="true"
      />
      {{ t('workspace.modelPreload.error') }}
    </p>

    <WorkspaceChatFab :instance="instanceName" />
  </main>
</template>
