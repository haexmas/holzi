<script setup lang="ts">
/**
 * The category "Darstellung" (spec 023-settings-app, FR-013, T039): the color scheme for this
 * device and for all devices, saved on selection (FR-021). "Wie alle Geräte" and "System" for
 * the vault clear the value instead of storing one, so the view never stores `system` for the
 * vault (contracts §3). The state comes from `useColorScheme`, which the workspace loads on open.
 */
import type { ColorScheme } from '~/lib/settings/colorScheme'

const { t } = useI18n()
const { errString } = useErrorString()
const { state } = useColorScheme()
const setScheme = useActionOrThrow('settings.appearance.setColorScheme')
const clearScheme = useActionOrThrow('settings.appearance.clearColorScheme')

const SCHEMES: readonly ColorScheme[] = ['light', 'dark', 'system']

const busy = ref(false)
const savedFlash = ref(false)
const opError = ref<string | null>(null)

/** `''` = no value of its own: "Wie alle Geräte" on the device, "System" for the vault. */
function selected(scope: 'device' | 'vault'): string {
  const value = state.value[scope]
  return scope === 'vault' && value === 'system' ? '' : (value ?? '')
}

async function chooseAsync(scope: 'device' | 'vault', event: Event) {
  const value = (event.target as HTMLSelectElement).value
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    if (value === '') await clearScheme({ scope })
    else await setScheme({ scope, scheme: value })
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
    <SettingsGroup :label="t('settings.colorScheme.label')">
      <SettingsRow
        v-for="scope in ['device', 'vault'] as const"
        :key="scope"
        :title="t(`settings.default.${scope}Label`)"
        :label-for="`settings-color-scheme-${scope}`"
      >
        <select
          :id="`settings-color-scheme-${scope}`"
          class="h-9 w-56 max-w-full rounded-md border border-input bg-background px-2 text-sm focus:ring-2 focus:ring-ring focus:outline-none"
          :value="selected(scope)"
          :disabled="busy"
          :data-testid="`settings-color-scheme-${scope}`"
          @change="chooseAsync(scope, $event)"
        >
          <option value="">
            {{
              scope === 'device'
                ? t('settings.colorScheme.followVault')
                : t('settings.colorScheme.system')
            }}
          </option>
          <option
            v-for="scheme in SCHEMES.filter(
              (s) => scope === 'device' || s !== 'system',
            )"
            :key="scheme"
            :value="scheme"
          >
            {{ t(`settings.colorScheme.${scheme}`) }}
          </option>
        </select>
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
