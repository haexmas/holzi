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
  <div class="relative min-w-0 shrink-0">
    <Icon
      v-if="icon"
      :name="icon"
      class="pointer-events-none absolute left-2 top-1/2 z-10 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground @lg/composer:hidden"
      :aria-hidden="true"
    />
    <UiSelect
      :id="controlId"
      :model-value="value || undefined"
      :options="options"
      :aria-label="ariaLabel"
      :disabled="disabled"
      :class="[
        'w-auto max-w-48',
        '[&_button]:h-7 [&_button]:w-auto [&_button]:max-w-48 [&_button]:shrink-0 [&_button]:gap-1.5 [&_button]:rounded-lg [&_button]:border-border/70 [&_button]:bg-muted/20 [&_button]:px-2 [&_button]:py-1 [&_button]:text-xs [&_button]:shadow-none',
        '[&_button]:pl-8 @lg/composer:[&_button]:pl-2',
        '[&_button>span]:hidden @lg/composer:[&_button>span]:block',
      ]"
      @update:model-value="updateValue"
    />
  </div>
</template>
