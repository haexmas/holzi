<script setup lang="ts">
/**
 * The category "Darstellung" (spec 023-settings-app, FR-013, FR-024): the color scheme of the
 * vault, saved on selection (FR-021) through `settings.appearance.setColorScheme`. The value
 * comes from `useColorScheme`, which the workspace loads on open.
 */
import type { SettingsSelectOption } from '~/components/settings/Select.vue'

const { t } = useI18n()
const { errString } = useErrorString()
const { scheme } = useColorScheme()
const setScheme = useActionOrThrow('settings.appearance.setColorScheme')

const options = computed<SettingsSelectOption[]>(() =>
  (['light', 'dark', 'system'] as const).map((value) => ({
    value,
    label: t(`settings.colorScheme.${value}`),
  })),
)

const busy = ref(false)
const savedFlash = ref(false)
const opError = ref<string | null>(null)

async function chooseAsync(value: string) {
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    await setScheme({ scheme: value })
    savedFlash.value = true
  } catch (e) {
    opError.value = errString(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="flex flex-col gap-2">
    <SettingsGroup>
      <SettingsRow
        :title="t('settings.colorScheme.label')"
        label-for="settings-color-scheme"
      >
        <SettingsSelect
          id="settings-color-scheme"
          :model-value="scheme"
          :options="options"
          :disabled="busy"
          data-testid="settings-color-scheme"
          @update:model-value="chooseAsync"
        />
      </SettingsRow>
    </SettingsGroup>

    <p v-if="savedFlash" class="px-1 text-xs text-success" role="status">
      {{ t('settings.colorScheme.saved') }}
    </p>
    <p v-if="opError" class="px-1 text-xs text-destructive" role="alert">
      {{ t('settings.colorScheme.failed') }}: {{ opError }}
    </p>
  </section>
</template>
