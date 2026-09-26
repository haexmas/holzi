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
  <section class="flex flex-col gap-3">
    <h2 id="session-restore-title" class="text-xl font-semibold">
      {{ t('settings.sessionRestore.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.sessionRestore.description') }}
    </p>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <template v-if="restore">
      <fieldset
        class="flex flex-col gap-1"
        aria-labelledby="session-restore-title"
        data-testid="session-restore-choice"
      >
        <label
          v-for="option in CHOICES"
          :key="option"
          class="flex items-center gap-2 text-sm"
        >
          <input
            type="radio"
            name="session-restore"
            :value="option"
            :checked="choice === option"
            :disabled="busy"
            :data-testid="`session-restore-${option}`"
            @change="chooseAsync(option)"
          />
          {{ t(`settings.sessionRestore.choice.${option}`) }}
        </label>
      </fieldset>

      <p v-if="savedFlash" class="text-xs text-green-600" role="status">
        {{ t('settings.sessionRestore.saved') }}
      </p>
      <p v-if="opError" class="text-xs text-red-500" role="alert">
        {{ t('settings.sessionRestore.failed') }}: {{ opError }}
      </p>
    </template>
  </section>
</template>
