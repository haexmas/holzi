<script setup lang="ts">
interface Props {
  open: boolean
  title?: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
}>()

function close() {
  emit('update:open', false)
}
</script>

<template>
  <Teleport to="body">
    <div v-if="props.open" class="fixed inset-0 z-50 flex items-center justify-center">
      <div
        class="absolute inset-0 bg-black/50"
        @click="close"
      />
      <div
        class="relative z-10 w-full max-w-md rounded-lg border shadow-lg p-6 mx-4"
        style="background-color: var(--color-card); color: var(--color-card-foreground); border-color: var(--color-border);"
        role="dialog"
        aria-modal="true"
      >
        <header v-if="props.title" class="mb-4 flex items-center justify-between">
          <h2 class="text-lg font-semibold">
            {{ props.title }}
          </h2>
          <button
            class="text-sm opacity-60 hover:opacity-100"
            aria-label="close"
            @click="close"
          >
            ✕
          </button>
        </header>
        <slot />
      </div>
    </div>
  </Teleport>
</template>
