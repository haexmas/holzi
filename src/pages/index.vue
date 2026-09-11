<script setup lang="ts">
const { t } = useI18n()
const store = useInstancesStore()

const createSheetOpen = ref(false)
const unlockSheetOpen = ref(false)
const selectedName = ref<string | null>(null)

onMounted(async () => {
  let listenerError: unknown
  try {
    await store.startListening()
  }
  catch (e) {
    listenerError = e
  }
  await store.syncAsync()
  if (listenerError !== undefined) {
    store.lastError = listenerError instanceof Error ? listenerError.message : String(listenerError)
  }
})

onBeforeUnmount(() => {
  store.stopListening()
})

/** Opens the unlock sheet for the selected instance. */
function onSelect(name: string) {
  selectedName.value = name
  unlockSheetOpen.value = true
}

/** Activates a newly created instance and opens its workspace-landing. */
async function onCreated(name: string) {
  store.setActiveInstance(name)
  await navigateTo(`/workspace/${encodeURIComponent(name)}`)
}

/** Activates an unlocked instance and opens its workspace-landing. */
async function onUnlocked(name: string) {
  store.setActiveInstance(name)
  await navigateTo(`/workspace/${encodeURIComponent(name)}`)
}
</script>

<template>
  <main class="min-h-screen flex flex-col items-center justify-center gap-6 p-8">
    <div class="flex flex-col items-center gap-2">
      <h1 class="text-3xl font-semibold">
        {{ t('landing.welcome') }}
      </h1>
    </div>

    <div class="flex flex-col gap-2 w-full max-w-md">
      <UiButton class="w-full" @click="createSheetOpen = true">
        {{ t('onboarding.create.title') }}
      </UiButton>
    </div>

    <OnboardingInstancesList
      :instances="store.instances"
      @select="onSelect"
    />

    <p v-if="store.lastError" class="text-sm text-red-500" role="status">
      {{ store.lastError }}
    </p>

    <OnboardingCreateSheet
      v-model:open="createSheetOpen"
      @created="onCreated"
    />

    <OnboardingUnlockSheet
      v-model:open="unlockSheetOpen"
      :name="selectedName"
      @unlocked="onUnlocked"
    />
  </main>
</template>
