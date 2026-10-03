<script setup lang="ts">
const { t } = useI18n()
const fieldLabels = useFieldLabels()

const props = defineProps<{
  hostnameHint: string | null
  modelValue: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  next: []
}>()

const localValue = ref(
  props.modelValue ||
    props.hostnameHint ||
    t('onboarding.alias.defaultPlaceholder'),
)
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
    <p class="text-sm text-muted-foreground">
      {{ t('onboarding.alias.description') }}
    </p>
    <UiInput
      :model-value="localValue"
      :label="t('onboarding.alias.label')"
      :labels="fieldLabels.input.value"
      :error="
        showRequired && !localValue.trim()
          ? t('onboarding.alias.required')
          : undefined
      "
      type="text"
      @update:model-value="localValue = String($event ?? '')"
      @input="showRequired = false"
    />
    <div class="flex justify-end">
      <UiButton type="submit">
        {{ t('onboarding.wizard.next') }}
      </UiButton>
    </div>
  </form>
</template>
