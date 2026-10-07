<script setup lang="ts">
const { t } = useI18n()
const language = useLanguage()
const { language: activeLanguage, options: languageOptions } = language
const store = useInstancesStore()
const { activeNameAsync } = useInstance()

const createSheetOpen = ref(false)
const linkSheetOpen = ref(false)
const unlockSheetOpen = ref(false)
const selectedName = ref<string | null>(null)

/** Re-syncs so a vault created or closed by another app process appears without a restart (spec
 * 013 US4, FR-021: this process's own `instance-list-changed` listener above only ever hears
 * about its own local changes). */
function onFocusOrVisible() {
  if (document.visibilityState === 'visible') void store.syncAsync()
}

onMounted(async () => {
  window.addEventListener('focus', onFocusOrVisible)
  document.addEventListener('visibilitychange', onFocusOrVisible)
  let listenerError: unknown
  try {
    await store.startListening()
  } catch (e) {
    listenerError = e
  }

  let activeName: string | null = null
  try {
    activeName = await activeNameAsync()
  } catch (e) {
    listenerError ??= e
  }
  if (activeName) {
    store.setActiveInstance(activeName)
    await navigateTo(`/workspace/${encodeURIComponent(activeName)}`, {
      replace: true,
    })
    return
  }

  await store.syncAsync()
  if (listenerError !== undefined) {
    store.lastError =
      listenerError instanceof Error
        ? listenerError.message
        : String(listenerError)
  }
})

onBeforeUnmount(() => {
  store.stopListening()
  window.removeEventListener('focus', onFocusOrVisible)
  document.removeEventListener('visibilitychange', onFocusOrVisible)
})

/** Opens the unlock sheet for the selected instance. */
function onSelect(name: string) {
  selectedName.value = name
  unlockSheetOpen.value = true
  void store.syncAsync()
}

/** Spec 042 (FR-007): the start page's choice lasts until the app restarts; nothing is stored. */
function chooseLanguage(value: string) {
  void language.showAsync(value)
}

/** Spec 042 (FR-009): the vault's language wins, a vault without one stores the active one. A read
 * error never keeps the vault from opening. */
async function applyVaultLanguageAsync() {
  try {
    await language.loadAsync()
  } catch (error) {
    console.error('[settings] reading the language failed', error)
  }
}

/** Activates a newly created instance and opens its workspace-landing. */
async function onCreated(name: string) {
  store.setActiveInstance(name)
  await applyVaultLanguageAsync()
  await navigateTo(`/workspace/${encodeURIComponent(name)}`, { replace: true })
}

/** A link finished: the new vault exists, so it appears in the list and opens like any other, with
 * the passphrase chosen for it. */
async function onLinked(name: string) {
  await store.syncAsync()
  onSelect(name)
}

/** Activates an unlocked instance and opens its workspace-landing. */
async function onUnlocked(name: string) {
  store.setActiveInstance(name)
  await applyVaultLanguageAsync()
  await navigateTo(`/workspace/${encodeURIComponent(name)}`, { replace: true })
}
</script>

<template>
  <main
    class="relative min-h-screen flex flex-col items-center justify-center gap-6 p-8"
  >
    <SettingsSelect
      class="absolute top-4 right-4 w-36"
      :model-value="activeLanguage"
      :options="languageOptions"
      :aria-label="t('landing.language')"
      data-testid="landing-language"
      @update:model-value="chooseLanguage"
    />
    <div class="flex flex-col items-center gap-2">
      <h1 class="text-3xl font-semibold">
        {{ t('landing.welcome') }}
      </h1>
    </div>

    <div class="flex flex-col gap-2 w-full max-w-md">
      <UiButton class="w-full" @click="createSheetOpen = true">
        {{ t('onboarding.create.title') }}
      </UiButton>
      <UiButton
        class="w-full"
        variant="outline"
        data-testid="landing-link"
        @click="linkSheetOpen = true"
      >
        {{ t('onboarding.link.title') }}
      </UiButton>
    </div>

    <OnboardingInstancesList :instances="store.instances" @select="onSelect" />

    <p v-if="store.lastError" class="text-sm text-destructive" role="status">
      {{ store.lastError }}
    </p>

    <OnboardingCreateSheet
      v-model:open="createSheetOpen"
      @created="onCreated"
    />

    <OnboardingLinkSheet v-model:open="linkSheetOpen" @linked="onLinked" />

    <OnboardingUnlockSheet
      v-model:open="unlockSheetOpen"
      :name="selectedName"
      @unlocked="onUnlocked"
    />
  </main>
</template>
