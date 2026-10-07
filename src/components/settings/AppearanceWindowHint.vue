<script setup lang="ts">
/**
 * The switch "Akzentfarbe als Hinweis für das aktive Fenster" of "Erscheinungsbild" (spec
 * 035-appearance-and-fields, FR-024): saved as soon as it flips, through `settings.appearance.set`.
 * Only the border of the active window changes; no token does.
 */
const emit = defineEmits<{ saved: []; failed: [message: string] }>()

const { t } = useI18n()
const { errString } = useErrorString()
const appearance = useAppearance()
const setAppearance = useActionOrThrow('settings.appearance.set')
const busy = ref(false)

async function toggleAsync(windowHint: boolean) {
  if (busy.value) return
  busy.value = true
  try {
    await setAppearance({ windowHint })
    emit('saved')
  } catch (error: unknown) {
    emit('failed', errString(error))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <SettingsRow
    :title="t('settings.appearance.windowHint')"
    label-for="appearance-window-hint"
  >
    <ShadcnSwitch
      id="appearance-window-hint"
      :model-value="appearance.windowHint.value"
      :disabled="busy"
      data-testid="appearance-window-hint"
      @update:model-value="toggleAsync($event === true)"
    />
  </SettingsRow>
</template>
