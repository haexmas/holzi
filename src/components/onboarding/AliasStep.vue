<script setup lang="ts">
const { t } = useI18n()

const props = defineProps<{
  hostnameHint: string | null
  modelValue: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  next: []
}>()

const localValue = ref(props.modelValue || props.hostnameHint || t('onboarding.alias.defaultPlaceholder'))
const showRequired = ref(false)

watch(localValue, (v) => emit('update:modelValue', v))

function onSubmit() {
  const trimmed = localValue.value.trim()
  if (!trimmed) {
    showRequired.value = true
    return
  }
  emit('update:modelValue', trimmed)
  emit('next')
}
</script>

<template>
  <form class="flex flex-col gap-3" @submit.prevent="onSubmit">
    <h2 class="text-xl font-semibold">
      {{ t('onboarding.alias.label') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('onboarding.alias.description') }}
    </p>
    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium">{{ t('onboarding.alias.label') }}</span>
      <input
        v-model="localValue"
        type="text"
        class="border border-neutral-300 rounded-md p-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
        :aria-invalid="showRequired && !localValue.trim() ? true : undefined"
        @input="showRequired = false"
      >
      <span v-if="showRequired && !localValue.trim()" class="text-xs text-red-500">
        {{ t('onboarding.alias.required') }}
      </span>
    </label>
    <div class="flex justify-end">
      <UiButton type="submit">
        {{ t('onboarding.wizard.next') }}
      </UiButton>
    </div>
  </form>
</template>
