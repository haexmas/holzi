<script setup lang="ts">
/**
 * This device's name (spec 002 US3). Saved when the field loses focus or on Enter, without a
 * save button (spec 023 FR-021); an empty name is not saved and the field says why.
 */
const { t } = useI18n()
const { errString } = useErrorString()
const setAlias = useActionOrThrow('settings.device.setAlias')

const props = defineProps<{
  currentAlias: string
}>()

const emit = defineEmits<{
  saved: [alias: string]
}>()

const localValue = ref(props.currentAlias)
const showRequired = ref(false)
const busy = ref(false)
const savedFlash = ref(false)
const saveError = ref<string | null>(null)

watch(
  () => props.currentAlias,
  (v) => {
    localValue.value = v
  },
)

/**
 * Clears the three transient field states as soon as the operator types.
 *
 * A named handler rather than a multi-statement template expression: Vue
 * collapses a template attribute's newlines before parsing it, so the
 * inline form needs `;` separators that Prettier's `semi: false` strips,
 * leaving markup the SFC compiler rejects.
 */
function onInput() {
  showRequired.value = false
  savedFlash.value = false
  saveError.value = null
}

async function commitAsync() {
  const trimmed = localValue.value.trim()
  if (!trimmed) {
    showRequired.value = true
    return
  }
  if (trimmed === props.currentAlias || busy.value) return
  busy.value = true
  savedFlash.value = false
  saveError.value = null
  try {
    await setAlias({ alias: trimmed })
    localValue.value = trimmed
    savedFlash.value = true
    emit('saved', trimmed)
  } catch (e) {
    saveError.value = errString(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="flex flex-col gap-3">
    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium">{{ t('settings.alias.label') }}</span>
      <input
        v-model="localValue"
        type="text"
        class="rounded-md border border-input bg-background p-2 focus:outline-none focus:ring-2 focus:ring-ring"
        :disabled="busy"
        :aria-invalid="showRequired && !localValue.trim() ? true : undefined"
        data-testid="settings-alias"
        @input="onInput"
        @blur="commitAsync"
        @keydown.enter.prevent="commitAsync"
      />
      <span
        v-if="showRequired && !localValue.trim()"
        class="text-xs text-destructive"
        role="alert"
      >
        {{ t('settings.alias.required') }}
      </span>
    </label>
    <span v-if="savedFlash" class="text-xs text-success" role="status">
      {{ t('settings.alias.saved') }}
    </span>
    <span v-if="saveError" class="text-xs text-destructive" role="alert">
      {{ t('errors.aliasSaveFailed') }}: {{ saveError }}
    </span>
  </section>
</template>
