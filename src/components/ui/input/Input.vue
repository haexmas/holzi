<script setup lang="ts">
import { cn } from '~/lib/utils'

interface Props {
  modelValue: string
  type?: string
  placeholder?: string
  class?: string
  autofocus?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  type: 'text',
})

defineEmits<{
  'update:modelValue': [value: string]
}>()
</script>

<template>
  <input
    :value="props.modelValue"
    :type="props.type"
    :placeholder="props.placeholder"
    :autofocus="props.autofocus"
    :class="cn(
      'flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
      props.class,
    )"
    @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
  >
</template>
