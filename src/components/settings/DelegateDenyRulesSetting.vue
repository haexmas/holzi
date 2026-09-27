<script setup lang="ts">
/**
 * Delegate deny rules on this device (spec 009). Each checkbox saves on change (spec 023
 * FR-021); a failure restores the stored selection.
 */

const { t } = useI18n()
const { errString } = useErrorString()
const { getPrefAsync } = usePreferences()
const setDenyRules = useActionOrThrow('settings.delegate.setDenyRules')

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

/** Reloads and validates the vault's deny-rule selection (spec 023 FR-024). */
async function reloadAsync() {
  loading.value = true
  loadError.value = null
  try {
    const raw = await getPrefAsync({ kind: 'vault' }, PREF_KEY)
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

/** Applies one checkbox change and saves the selection for the vault. */
async function toggleAsync(category: string, checked: boolean) {
  const previous = selected.value
  const next = new Set(previous)
  if (checked) next.add(category)
  else next.delete(category)
  selected.value = next
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    await setDenyRules({ rules: [...next] })
    savedFlash.value = true
  } catch (e) {
    opError.value = errString(e)
    selected.value = previous
  } finally {
    busy.value = false
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-2">
    <div v-if="loading" class="text-sm text-muted-foreground">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-destructive" role="alert">
      {{ t('settings.denyRules.loadFailed') }}: {{ loadError }}
    </p>

    <template v-if="!loading">
      <fieldset :aria-label="t('settings.locations.agents.denyRules.title')">
        <SettingsGroup>
          <SettingsOptionRow
            v-for="category in CATEGORIES"
            :key="category"
            type="checkbox"
            :checked="selected.has(category)"
            :disabled="busy"
            :title="t(`settings.denyRules.${category}`)"
            :description="t(`settings.denyRules.${category}Description`)"
            :data-testid="`settings-deny-${category}`"
            @change="(checked) => toggleAsync(category, checked)"
          />
        </SettingsGroup>
      </fieldset>

      <p v-if="savedFlash" class="px-1 text-xs text-success" role="status">
        {{ t('settings.denyRules.saved') }}
      </p>
      <p v-if="opError" class="px-1 text-xs text-destructive" role="alert">
        {{ t('settings.denyRules.saveFailed') }}: {{ opError }}
      </p>
    </template>
  </section>
</template>
