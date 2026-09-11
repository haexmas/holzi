<script setup lang="ts">
// Minimal workspace-landing stub for spec 002. Deliberately barebones
// in this feature — the full workspace build-out (widgets, panels) is
// a separate feature. Provides the instance heading + persistent FAB
// that routes to the existing chat page.
definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const { t } = useI18n()

const instanceName = computed(() => {
  const raw = route.params.instance
  return typeof raw === 'string' ? decodeURIComponent(raw) : Array.isArray(raw) ? decodeURIComponent(raw[0] ?? '') : ''
})
</script>

<template>
  <main class="min-h-screen flex flex-col p-6">
    <header class="flex items-center justify-between">
      <h1 class="text-2xl font-semibold">
        {{ t('workspace.heading', { instance: instanceName }) }}
      </h1>
    </header>

    <WorkspaceChatFab :instance="instanceName" />
  </main>
</template>
