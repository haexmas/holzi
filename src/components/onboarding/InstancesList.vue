<script setup lang="ts">
import type { InstanceInfo } from '@bindings/InstanceInfo'

interface Props {
  instances: InstanceInfo[]
}

const props = defineProps<Props>()
const emit = defineEmits<{
  select: [name: string]
}>()

const { t } = useI18n()

function formatRelative(ms: number): string {
  if (ms === 0)
    return '—'
  const d = new Date(ms)
  return d.toLocaleString()
}
</script>

<template>
  <section v-if="props.instances.length > 0" class="w-full max-w-md">
    <h2 class="text-sm font-medium mb-2 text-muted-foreground">
      {{ t('landing.lastUsed') }}
    </h2>
    <ul class="space-y-1">
      <li v-for="i in props.instances" :key="i.name">
        <button
          class="w-full text-left rounded-md border px-3 py-2 hover:bg-accent hover:text-accent-foreground transition-colors"
          style="border-color: var(--color-border);"
          @click="emit('select', i.name)"
        >
          <div class="font-medium">
            {{ i.alias ?? i.name }}
          </div>
          <div class="text-xs text-muted-foreground">
            {{ formatRelative(i.lastAccess) }}
          </div>
        </button>
      </li>
    </ul>
  </section>
</template>
