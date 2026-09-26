<script setup lang="ts">
/**
 * Default autonomy mode for delegates on this device (spec 009). The options save on selection
 * (spec 023 FR-021); choosing a mode is the explicit opt-in spec 009 asks for.
 */
import {
  AUTONOMY_MODES,
  isAutonomyMode,
  type AutonomyMode,
} from '~/composables/usePreferences'

const { t } = useI18n()
const { errString } = useErrorString()
const { getPrefAsync } = usePreferences()
const setMode = useActionOrThrow('settings.autonomy.setMode')

// No Rust-side setter exists for this preference, same as
// `chat.permission_mode` and `cli_delegate.deny_rules` — the frontend
// writes it directly through the generic `set_pref` command.
const PREF_KEY = 'chat.autonomy_mode'

const MODES = AUTONOMY_MODES
const DEFAULT_MODE: AutonomyMode = 'ungated'

const selected = ref<AutonomyMode>(DEFAULT_MODE)
const stored = ref<AutonomyMode>(DEFAULT_MODE)
const loading = ref(true)
const busy = ref(false)
const savedFlash = ref(false)
const opError = ref<string | null>(null)
const loadError = ref<string | null>(null)

/** Reloads the device-scoped autonomy default, falling back to 'ungated'. */
async function reloadAsync() {
  loading.value = true
  loadError.value = null
  try {
    const raw = await getPrefAsync({ kind: 'vault' }, PREF_KEY)
    stored.value = isAutonomyMode(raw) ? raw : DEFAULT_MODE
    selected.value = stored.value
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
}

/** Persists the chosen autonomy default for this device; a failure restores the stored one. */
async function chooseAsync(mode: AutonomyMode) {
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    await setMode({ mode })
    stored.value = mode
    savedFlash.value = true
  } catch (e) {
    opError.value = errString(e)
    selected.value = stored.value
  } finally {
    busy.value = false
  }
}

/** Shows the choice at once; `chooseAsync` puts the stored one back if saving fails. */
function onChoose(mode: AutonomyMode) {
  selected.value = mode
  void chooseAsync(mode)
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-2">
    <div v-if="loading" class="text-sm text-muted-foreground">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-destructive" role="alert">
      {{ t('settings.autonomyMode.loadFailed') }}: {{ loadError }}
    </p>

    <fieldset
      v-if="!loading"
      :aria-label="t('settings.locations.agents.autonomy.title')"
    >
      <SettingsGroup>
        <SettingsOptionRow
          v-for="mode in MODES"
          :key="mode"
          type="radio"
          name="autonomy-mode"
          :value="mode"
          :checked="selected === mode"
          :disabled="busy"
          :title="t(`chat.autonomy.${mode}`)"
          :description="t(`settings.autonomyMode.${mode}Description`)"
          @change="onChoose(mode)"
        />
      </SettingsGroup>
    </fieldset>

    <p v-if="savedFlash" class="px-1 text-xs text-success" role="status">
      {{ t('settings.autonomyMode.saved') }}
    </p>
    <p v-if="opError" class="px-1 text-xs text-destructive" role="alert">
      {{ t('settings.autonomyMode.saveFailed') }}: {{ opError }}
    </p>
  </section>
</template>
