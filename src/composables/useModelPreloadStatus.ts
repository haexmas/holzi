import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type {
  ModelLoadErrorEvent,
  ModelLoadProgressEvent,
  ModelLoadStatusPayload,
  useChat,
} from '~/composables/useChat'

/**
 * Model preload/readiness status for the Shell status bar (spec
 * 015-workspace-shell, T020, FR-005) — moved unchanged from the former
 * workspace stub page (`pages/workspace/[instance].vue`, spec 002).
 * Registers and tears down its own `chat.onModelLoad*` listeners against
 * whichever component calls this (the Shell host), so it works
 * independently of which (or whether any) window is open.
 */
export function useModelPreloadStatus(chat: ReturnType<typeof useChat>) {
  const preloadStatus = ref<ModelLoadStatusPayload | null>(null)
  const preloadError = ref(false)
  const unlisteners: UnlistenFn[] = []
  let unmounted = false

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
        // The Shell remains usable even when the status snapshot is unavailable.
      }
    }
  })

  onBeforeUnmount(() => {
    unmounted = true
    for (const unlisten of unlisteners.splice(0)) unlisten()
  })

  return { preloadStatus, preloadError }
}
