<script setup lang="ts">
// Placeholder app, moved into the window manager unchanged (spec 015-workspace-shell,
// T022, FR-003: "existing placeholder content").
const instancesStore = useInstancesStore()
const wm = useWindowManagerStore()
const { t } = useI18n()
const { closeAsync } = useInstance()

const name = computed(() => instancesStore.activeInstance ?? '')

// Flushes the window manager layout (FR-027) first, same as ChatApp.vue's lock(). The backend then
// replaces this page with a spinner and ends the process (spec 013), so nothing else is navigated
// or cleared here and a failed call has nothing to show.
async function onLock() {
  await wm.flushAsync()
  await closeAsync().catch(() => {})
}
</script>

<template>
  <main
    class="h-full min-h-0 flex flex-col items-center justify-center gap-6 p-8"
  >
    <h1 class="text-2xl font-semibold">
      {{ name }}
    </h1>
    <p class="text-sm text-muted-foreground">
      Federation view — not this spec.
    </p>
    <UiButton variant="outline" @click="onLock">
      {{ t('onboarding.lock.submit') }}
    </UiButton>
  </main>
</template>
