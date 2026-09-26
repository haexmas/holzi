<script setup lang="ts">
const { t } = useI18n()
const { errString } = useErrorString()
const { getPrefAsync } = usePreferences()
const setDenyRules = useActionOrThrow('settings.delegate.setDenyRules')

const props = defineProps<{
  deviceUuid: string
}>()

// No Rust-side setter exists for this preference (research.md §5) — the
// frontend writes the JSON array of category identifiers directly through
// the existing generic `set_pref` command, exactly like
// `chat.permission_mode` has no dedicated command of its own either.
const PREF_KEY = 'cli_delegate.deny_rules'

const CATEGORIES = [
  'workspace_escape',
  'network_access',
  'credential_paths',
] as const
const VALID_CATEGORIES = new Set<string>(CATEGORIES)

const selected = ref<Set<string>>(new Set())
const loading = ref(true)
const busy = ref(false)
const savedFlash = ref(false)
const opError = ref<string | null>(null)
const loadError = ref<string | null>(null)

/** Reloads and validates the device-scoped deny-rule selection. */
async function reloadAsync() {
  loading.value = true
  loadError.value = null
  try {
    const raw = await getPrefAsync(
      { kind: 'device', uuid: props.deviceUuid },
      PREF_KEY,
    )
    const parsed: unknown = raw ? JSON.parse(raw) : []
    const validItems = Array.isArray(parsed)
      ? parsed.filter(
          (item): item is string =>
            typeof item === 'string' && VALID_CATEGORIES.has(item),
        )
      : []
    selected.value = new Set(validItems)
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
}

/** Applies one checkbox change to the local selection. */
function toggle(category: string, checked: boolean) {
  const next = new Set(selected.value)
  if (checked) next.add(category)
  else next.delete(category)
  selected.value = next
  savedFlash.value = false
}

/** Persists the current deny-rule selection for this device. */
async function onSave() {
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    await setDenyRules({ rules: [...selected.value] })
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
      {{ t('settings.denyRules.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.denyRules.description') }}
    </p>

    <div v-if="loading" class="text-sm text-neutral-500">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('settings.denyRules.loadFailed') }}: {{ loadError }}
    </p>

    <template v-if="!loading">
      <fieldset class="flex flex-col gap-2">
        <label
          v-for="category in CATEGORIES"
          :key="category"
          class="flex items-center gap-2 text-sm"
        >
          <input
            type="checkbox"
            :checked="selected.has(category)"
            :disabled="busy"
            @change="
              toggle(category, ($event.target as HTMLInputElement).checked)
            "
          />
          {{ t(`settings.denyRules.${category}`) }}
          <span class="text-neutral-500">
            — {{ t(`settings.denyRules.${category}Description`) }}
          </span>
        </label>
      </fieldset>

      <div class="flex items-center gap-3 flex-wrap">
        <UiButton type="button" :disabled="busy" @click="onSave">
          {{ t('settings.denyRules.save') }}
        </UiButton>
        <span v-if="savedFlash" class="text-xs text-green-600" role="status">
          {{ t('settings.denyRules.saved') }}
        </span>
        <span v-if="opError" class="text-xs text-red-500" role="alert">
          {{ t('settings.denyRules.saveFailed') }}: {{ opError }}
        </span>
      </div>
    </template>
  </section>
</template>
