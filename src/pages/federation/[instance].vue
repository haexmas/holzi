<script setup lang="ts">
const route = useRoute()
const { t } = useI18n()
const { closeAsync } = useInstance()
const store = useInstancesStore()

const name = computed(() => String(route.params.instance ?? ''))

async function onLock() {
  try {
    await closeAsync()
    store.setActiveInstance(null)
    await navigateTo('/')
  }
  catch (e) {
    // eslint-disable-next-line no-console
    console.error('close failed', e)
  }
}
</script>

<template>
  <main class="min-h-screen flex flex-col items-center justify-center gap-6 p-8">
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
