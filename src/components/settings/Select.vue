<script setup lang="ts">
/**
 * The select of a settings row (spec 023-settings-app, FR-013): haex-ui's `ShadcnSelect`, so it
 * is readable in both color schemes. Reka's items cannot carry `''`, so an empty value ("Keins",
 * "Wie alle Geräte") travels as a sentinel inside and as `''` outside. Attributes such as `id` and
 * `data-testid` go to the trigger, which a row's `<label for>` then names; a `class` replaces the
 * trigger's default width.
 */
defineOptions({ inheritAttrs: false })

export type SettingsSelectOption = {
  value: string
  label: string
  disabled?: boolean
}

const props = defineProps<{
  modelValue: string
  options?: readonly SettingsSelectOption[]
  groups?: readonly {
    key: string
    label: string
    options: readonly SettingsSelectOption[]
  }[]
  disabled?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const attrs = useAttrs()
const triggerAttrs = computed(() => {
  const { class: _class, ...rest } = attrs
  return rest
})
const width = computed(() => (attrs.class as string | undefined) ?? 'w-56')

const NONE = '__none__'
const wire = (value: string) => (value === '' ? NONE : value)

function onUpdate(value: unknown) {
  const next = String(value)
  emit('update:modelValue', next === NONE ? '' : next)
}
</script>

<template>
  <ShadcnSelect
    :model-value="wire(props.modelValue)"
    :disabled="disabled"
    @update:model-value="onUpdate"
  >
    <ShadcnSelectTrigger
      v-bind="triggerAttrs"
      :class="['h-9 max-w-full bg-background text-sm', width]"
    >
      <ShadcnSelectValue />
    </ShadcnSelectTrigger>
    <ShadcnSelectContent class="max-h-[60vh]">
      <ShadcnSelectItem
        v-for="option in options ?? []"
        :key="option.value"
        :value="wire(option.value)"
        :disabled="option.disabled"
      >
        {{ option.label }}
      </ShadcnSelectItem>
      <ShadcnSelectGroup v-for="group in groups ?? []" :key="group.key">
        <ShadcnSelectLabel>{{ group.label }}</ShadcnSelectLabel>
        <ShadcnSelectItem
          v-for="option in group.options"
          :key="option.value"
          :value="wire(option.value)"
          :disabled="option.disabled"
        >
          {{ option.label }}
        </ShadcnSelectItem>
      </ShadcnSelectGroup>
    </ShadcnSelectContent>
  </ShadcnSelect>
</template>
