<script setup lang="ts">
const props = defineProps<{
  label: string
  value?: string
  displayValue?: string
  icon?: string
  disabled?: boolean
  controlId?: string
  options: { value: string; label: string }[]
}>()

const emit = defineEmits<{
  'update:value': [value: string]
}>()

const controlId = computed(
  () =>
    props.controlId ??
    `composer-control-${props.label.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`,
)

const ariaLabel = computed(() =>
  (props.displayValue ?? props.value)
    ? `${props.label}: ${props.displayValue ?? props.value}`
    : props.label,
)

function updateValue(nextValue: unknown) {
  if (typeof nextValue === 'string') emit('update:value', nextValue)
}
</script>

<template>
  <ShadcnSelect
    :model-value="value || undefined"
    :disabled="disabled"
    @update:model-value="updateValue"
  >
    <!-- One frame around icon, value and chevron; the value text yields to
         the icon alone when the composer is too narrow (`composer` container
         in Composer.vue). -->
    <ShadcnSelectTrigger
      :id="controlId"
      class="h-7 w-auto max-w-48 shrink-0 gap-1.5 rounded-lg border-border/70 bg-muted/20 px-2 py-1 text-xs shadow-none"
      :aria-label="ariaLabel"
      :title="ariaLabel"
    >
      <Icon
        v-if="icon"
        :name="icon"
        class="h-3.5 w-3.5 shrink-0 text-muted-foreground"
        :aria-hidden="true"
      />
      <ShadcnSelectValue
        class="hidden @lg/composer:block"
        :placeholder="displayValue ?? label"
      />
    </ShadcnSelectTrigger>
    <ShadcnSelectContent>
      <ShadcnSelectItem
        v-for="option in options"
        :key="option.value"
        :value="option.value"
      >
        {{ option.label }}
      </ShadcnSelectItem>
    </ShadcnSelectContent>
  </ShadcnSelect>
</template>
