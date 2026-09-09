<script setup lang="ts">
interface Props {
  for?: string
}

const props = defineProps<Props>()
</script>

<template>
  <label
    :for="props.for"
    class="text-sm font-medium leading-none block mb-1.5"
  >
    <slot />
  </label>
</template>
