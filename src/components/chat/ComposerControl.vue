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

function updateValue(nextValue: unknown) {
  if (typeof nextValue === 'string') emit('update:value', nextValue)
}
</script>

<template>
  <div
    class="flex min-w-0 shrink-0 items-center gap-1 rounded-xl border border-border/70 bg-muted/20 px-2 py-1"
  >
    <Icon
      v-if="icon"
      :name="icon"
      class="h-3.5 w-3.5 shrink-0 text-muted-foreground"
      :aria-hidden="true"
    />
    <div class="min-w-0 max-w-44">
      <UiSelect
        :id="controlId"
        :model-value="value || undefined"
        :options="options"
        :placeholder="displayValue ?? label"
        :aria-label="
          (displayValue ?? value) ? `${label}: ${displayValue ?? value}` : label
        "
        :disabled="disabled"
        @update:model-value="updateValue"
      />
    </div>
  </div>
</template>
