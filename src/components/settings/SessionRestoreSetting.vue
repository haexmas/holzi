<script setup lang="ts">
/**
 * The setting "Sitzung wiederherstellen" (spec 022-session-restore, FR-004,
 * contracts/wm-session.md §5): one choice — off, only this device, all devices
 * of the vault — saved as soon as it is picked, without a save button.
 * `restoreChoiceSteps` maps the choice onto the device and vault values.
 * Writes go through the catalog actions (spec 020 FR-024).
 */
import {
  fromRestoreResult,
  restoreChoice,
  restoreChoiceSteps,
  type RestoreChoice,
  type RestoreState,
  type RestoreStateResult,
} from '~/lib/wm/sessionSync'

const CHOICES: readonly RestoreChoice[] = ['off', 'device', 'vault']

const { t } = useI18n()
const { errString } = useErrorString()
const wm = useWindowManagerStore()
const setRestore = useActionOrThrow('settings.sessionRestore.set')
const clearRestore = useActionOrThrow('settings.sessionRestore.clear')

const restore = ref<RestoreState | null>(null)
const busy = ref(false)
const savedFlash = ref(false)
const loadError = ref<string | null>(null)
const opError = ref<string | null>(null)

const choice = computed(() =>
  restore.value ? restoreChoice(restore.value) : null,
)

async function reloadAsync() {
  try {
    restore.value = await wm.getSessionRestore()
    loadError.value = null
  } catch (error: unknown) {
    loadError.value = errString(error)
  }
}

async function chooseAsync(next: RestoreChoice) {
  if (!restore.value || busy.value) return
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    for (const step of restoreChoiceSteps(restore.value, next)) {
      const result =
        step.enabled === null
          ? await clearRestore({ scope: step.scope })
          : await setRestore({ scope: step.scope, enabled: step.enabled })
      restore.value = fromRestoreResult(result as RestoreStateResult)
    }
    savedFlash.value = true
  } catch (error: unknown) {
    opError.value = errString(error)
    await reloadAsync()
  } finally {
    busy.value = false
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-2">
    <p v-if="loadError" class="text-sm text-destructive" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <fieldset v-if="restore" data-testid="session-restore-choice">
      <legend class="mb-2 px-1 text-sm font-semibold">
        {{ t('settings.sessionRestore.title') }}
      </legend>
      <SettingsGroup>
        <SettingsOptionRow
          v-for="option in CHOICES"
          :key="option"
          type="radio"
          name="session-restore"
          :value="option"
          :checked="choice === option"
          :disabled="busy"
          :title="t(`settings.sessionRestore.choice.${option}`)"
          :data-testid="`session-restore-${option}`"
          @change="chooseAsync(option)"
        />
      </SettingsGroup>
    </fieldset>

    <p v-if="savedFlash" class="px-1 text-xs text-success" role="status">
      {{ t('settings.sessionRestore.saved') }}
    </p>
    <p v-if="opError" class="px-1 text-xs text-destructive" role="alert">
      {{ t('settings.sessionRestore.failed') }}: {{ opError }}
    </p>
  </section>
</template>
