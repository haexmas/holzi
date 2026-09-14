<script setup lang="ts">
const props = defineProps<{
  reasoning: string
  label: string
  expanded?: boolean
}>()

const emit = defineEmits<{
  'update:expanded': [expanded: boolean]
}>()

const expanded = computed({
  get: () => props.expanded ?? false,
  set: (value: boolean) => emit('update:expanded', value),
})
</script>

<template>
  <details
    class="mt-1 text-xs"
    :open="expanded"
    @toggle="expanded = ($event.target as HTMLDetailsElement).open"
  >
    <summary
      class="cursor-pointer list-none text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
      :aria-expanded="expanded"
    >
      <span class="inline-flex items-center gap-1 rounded px-1 py-0.5">
        <Icon name="lucide:brain" class="h-3.5 w-3.5" aria-hidden="true" />
        {{ label }}
        <Icon name="lucide:chevron-down" class="h-3.5 w-3.5" aria-hidden="true" />
      </span>
    </summary>
    <div class="mt-1 max-h-48 overflow-y-auto whitespace-pre-wrap rounded-lg bg-muted/30 px-2 py-1 text-muted-foreground">
      {{ reasoning }}
    </div>
  </details>
</template>
