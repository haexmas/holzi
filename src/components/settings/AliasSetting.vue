<script setup lang="ts">
const { t } = useI18n()
const { updateDeviceAliasAsync } = useDevice()

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

async function onSubmit() {
  const trimmed = localValue.value.trim()
  if (!trimmed) {
    showRequired.value = true
    return
  }
  busy.value = true
  savedFlash.value = false
  saveError.value = null
  try {
    await updateDeviceAliasAsync(trimmed)
    localValue.value = trimmed
    savedFlash.value = true
    emit('saved', trimmed)
  }
  catch (e) {
    saveError.value = e instanceof Error ? e.message : String(e)
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <form class="flex flex-col gap-3" @submit.prevent="onSubmit">
    <h2 class="text-xl font-semibold">
      {{ t('settings.alias.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.alias.description') }}
    </p>
    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium">{{ t('settings.alias.label') }}</span>
      <input
        v-model="localValue"
        type="text"
        class="border border-neutral-300 rounded-md p-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
        :disabled="busy"
        :aria-invalid="showRequired && !localValue.trim() ? true : undefined"
        @input="showRequired = false; savedFlash = false; saveError = null"
      >
      <span v-if="showRequired && !localValue.trim()" class="text-xs text-red-500">
        {{ t('settings.alias.required') }}
      </span>
    </label>
    <div class="flex items-center gap-3">
      <UiButton
        type="submit"
        :disabled="busy || !localValue.trim() || localValue.trim() === props.currentAlias"
      >
        {{ t('settings.alias.save') }}
      </UiButton>
      <span v-if="savedFlash" class="text-xs text-green-600" role="status">
        {{ t('settings.alias.saved') }}
      </span>
      <span v-if="saveError" class="text-xs text-red-500" role="alert">
        {{ t('errors.aliasSaveFailed') }}: {{ saveError }}
      </span>
    </div>
  </form>
</template>
