<script setup lang="ts">
import {
  AUTONOMY_MODES,
  isAutonomyMode,
  type AutonomyMode,
} from '~/composables/usePreferences'

const { t } = useI18n()
const { errString } = useErrorString()
const { getPrefAsync, setPrefAsync } = usePreferences()

const props = defineProps<{
  deviceUuid: string
}>()

// No Rust-side setter exists for this preference, same as
// `chat.permission_mode` and `cli_delegate.deny_rules` — the frontend
// writes it directly through the generic `set_pref` command.
const PREF_KEY = 'chat.autonomy_mode'

const MODES = AUTONOMY_MODES
const DEFAULT_MODE: AutonomyMode = 'ungated'

const selected = ref<AutonomyMode>(DEFAULT_MODE)
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
    const raw = await getPrefAsync(
      { kind: 'device', uuid: props.deviceUuid },
      PREF_KEY,
    )
    selected.value = isAutonomyMode(raw) ? raw : DEFAULT_MODE
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
}

/** Persists the selected autonomy default for this device. */
async function onSave() {
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    await setPrefAsync(
      { kind: 'device', uuid: props.deviceUuid },
      PREF_KEY,
      selected.value,
    )
    savedFlash.value = true
  } catch (e) {
    opError.value = errString(e)
  } finally {
    busy.value = false
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-3">
    <h2 class="text-xl font-semibold">
      {{ t('settings.autonomyMode.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.autonomyMode.description') }}
    </p>

    <div v-if="loading" class="text-sm text-neutral-500">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('settings.autonomyMode.loadFailed') }}: {{ loadError }}
    </p>

    <fieldset v-if="!loading" class="flex flex-col gap-2">
      <label
        v-for="mode in MODES"
        :key="mode"
        class="flex items-start gap-2 text-sm"
      >
        <input
          v-model="selected"
          type="radio"
          :value="mode"
          :disabled="busy"
          class="mt-1"
        />
        <span>
          <span class="font-medium">{{ t(`chat.autonomy.${mode}`) }}</span>
          <br />
          <span class="text-neutral-500">{{
            t(`settings.autonomyMode.${mode}Description`)
          }}</span>
        </span>
      </label>
    </fieldset>

    <div v-if="!loading" class="flex items-center gap-3 flex-wrap">
      <UiButton type="button" :disabled="busy" @click="onSave">
        {{ t('settings.autonomyMode.save') }}
      </UiButton>
      <span v-if="savedFlash" class="text-xs text-green-600" role="status">
        {{ t('settings.autonomyMode.saved') }}
      </span>
      <span v-if="opError" class="text-xs text-red-500" role="alert">
        {{ t('settings.autonomyMode.saveFailed') }}: {{ opError }}
      </span>
    </div>
  </section>
</template>
