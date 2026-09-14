<script setup lang="ts">
const props = defineProps<{
  label: string
  value?: string
  displayValue?: string
  icon?: string
  disabled?: boolean
  controlId?: string
}>()

const emit = defineEmits<{
  'update:value': [value: string]
}>()

const controlId = computed(() => props.controlId ?? `composer-control-${props.label.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`)
</script>

<template>
  <div class="flex min-w-0 shrink-0 items-center gap-1 rounded-xl border border-border/70 bg-muted/20 px-2 py-1">
    <Icon v-if="icon" :name="icon" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
    <label :for="controlId" class="sr-only">{{ label }}</label>
    <select
      :id="controlId"
      class="min-w-0 max-w-44 appearance-none bg-transparent px-1.5 py-0.5 text-xs font-medium text-foreground/80 outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
      :value="value"
      :disabled="disabled"
      :aria-label="(displayValue ?? value) ? `${label}: ${displayValue ?? value}` : label"
      @change="emit('update:value', ($event.target as HTMLSelectElement).value)"
    >
      <slot />
    </select>
    <Icon name="lucide:chevron-down" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
  </div>
</template>
