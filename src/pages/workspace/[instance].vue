<script setup lang="ts">
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

const instanceName = computed(() => {
  const raw = route.params.instance
  return typeof raw === 'string' ? raw : Array.isArray(raw) ? (raw[0] ?? '') : ''
})

const settingsTarget = computed(() => `/settings/${encodeURIComponent(instanceName.value)}`)
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

    <WorkspaceChatFab :instance="instanceName" />
  </main>
</template>
