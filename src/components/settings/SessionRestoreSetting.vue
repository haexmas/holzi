<script setup lang="ts">
/**
 * The setting "Sitzung wiederherstellen" (spec 022-session-restore, FR-004,
 * contracts/wm-session.md §5): device value, vault value and the value that
 * applies here, a scope choice like `DefaultModelSetting.vue`, and on/off/
 * reset for the chosen scope. A single switch could not show "not set".
 * Writes go through the catalog actions (spec 020 FR-024).
 */
import {
  fromRestoreResult,
  type RestoreState,
  type RestoreStateResult,
} from '~/lib/wm/sessionSync'

type ScopeKind = 'device' | 'vault'

const { t } = useI18n()
const { errString } = useErrorString()
const wm = useWindowManagerStore()
const setRestore = useActionOrThrow('settings.sessionRestore.set')
const clearRestore = useActionOrThrow('settings.sessionRestore.clear')

const restore = ref<RestoreState | null>(null)
const selectedScope = ref<ScopeKind>('device')
const busy = ref(false)
const savedFlash = ref(false)
const loadError = ref<string | null>(null)
const opError = ref<string | null>(null)

const valueForScope = computed(() =>
  restore.value ? restore.value[selectedScope.value] : null,
)

function label(value: boolean | null): string {
  if (value === null) return t('settings.sessionRestore.current.unset')
  return t(
    value
      ? 'settings.sessionRestore.current.on'
      : 'settings.sessionRestore.current.off',
  )
}

async function reloadAsync() {
  try {
    restore.value = await wm.getSessionRestore()
    loadError.value = null
  } catch (error: unknown) {
    loadError.value = errString(error)
  }
}

async function runAsync(change: () => Promise<unknown>) {
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    restore.value = fromRestoreResult((await change()) as RestoreStateResult)
    savedFlash.value = true
  } catch (error: unknown) {
    opError.value = errString(error)
  } finally {
    busy.value = false
  }
}

const turn = (enabled: boolean) =>
  runAsync(() => setRestore({ scope: selectedScope.value, enabled }))
const reset = () => runAsync(() => clearRestore({ scope: selectedScope.value }))

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-3">
    <h2 class="text-xl font-semibold">
      {{ t('settings.sessionRestore.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.sessionRestore.description') }}
    </p>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <template v-if="restore">
      <div class="flex flex-col gap-1 text-sm">
        <span>
          {{ t('settings.sessionRestore.current.device') }}:
          <strong>{{ label(restore.device) }}</strong>
        </span>
        <span>
          {{ t('settings.sessionRestore.current.vault') }}:
          <strong>{{ label(restore.vault) }}</strong>
        </span>
        <span data-testid="session-restore-effective">
          {{ t('settings.sessionRestore.current.effective') }}:
          <strong>{{ label(restore.effective) }}</strong>
        </span>
      </div>

      <fieldset class="flex flex-col gap-1">
        <legend class="text-sm font-medium">
          {{ t('settings.sessionRestore.scope.label') }}
        </legend>
        <label class="flex items-center gap-2 text-sm">
          <input
            v-model="selectedScope"
            type="radio"
            value="device"
            :disabled="busy"
          />
          {{ t('settings.sessionRestore.scope.device') }}
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input
            v-model="selectedScope"
            type="radio"
            value="vault"
            :disabled="busy"
          />
          {{ t('settings.sessionRestore.scope.vault') }}
        </label>
      </fieldset>

      <div class="flex items-center gap-3 flex-wrap">
        <UiButton
          type="button"
          data-testid="session-restore-enable"
          :disabled="busy || valueForScope === true"
          @click="turn(true)"
        >
          {{ t('settings.sessionRestore.enable') }}
        </UiButton>
        <UiButton
          type="button"
          variant="outline"
          :disabled="busy || valueForScope === false"
          @click="turn(false)"
        >
          {{ t('settings.sessionRestore.disable') }}
        </UiButton>
        <UiButton
          type="button"
          variant="outline"
          :disabled="busy || valueForScope === null"
          @click="reset()"
        >
          {{ t('settings.sessionRestore.reset') }}
        </UiButton>
        <span v-if="savedFlash" class="text-xs text-green-600" role="status">
          {{ t('settings.sessionRestore.saved') }}
        </span>
        <span v-if="opError" class="text-xs text-red-500" role="alert">
          {{ t('settings.sessionRestore.failed') }}: {{ opError }}
        </span>
      </div>
    </template>
  </section>
</template>
