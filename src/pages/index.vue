<script setup lang="ts">
const { t } = useI18n()
const store = useInstancesStore()

const createSheetOpen = ref(false)
const unlockSheetOpen = ref(false)
const selectedName = ref<string | null>(null)

onMounted(async () => {
  await store.startListening()
  await store.syncAsync()
})

onBeforeUnmount(() => {
  store.stopListening()
})

function onSelect(name: string) {
  selectedName.value = name
  unlockSheetOpen.value = true
}

async function onCreated(name: string) {
  store.setActiveInstance(name)
  await navigateTo(`/federation/${encodeURIComponent(name)}`)
}

async function onUnlocked(name: string) {
  store.setActiveInstance(name)
  await navigateTo(`/federation/${encodeURIComponent(name)}`)
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
