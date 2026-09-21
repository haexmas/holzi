<script setup lang="ts">
const route = useRoute()
const { t } = useI18n()
const { closeAsync } = useInstance()

const name = computed(() => String(route.params.instance ?? ''))

// The backend replaces this page with a spinner and ends the process (spec 013), so nothing is
// navigated or cleared here and a failed call has nothing to show.
async function onLock() {
  await closeAsync().catch(() => {})
}
</script>

<template>
  <main
    class="min-h-screen flex flex-col items-center justify-center gap-6 p-8"
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
