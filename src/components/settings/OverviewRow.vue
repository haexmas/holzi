<script setup lang="ts">
/**
 * One row that opens a sub-view (spec 023-settings-app, FR-003): icon, title, one-line
 * description and chevron. Navigates to `to` as a new tab location.
 */
defineProps<{
  to: string
  icon: string
  title: string
  description: string
}>()

const router = useTabRouter()
</script>

<template>
  <button
    type="button"
    class="flex w-full items-center gap-4 rounded-md px-3 py-3 text-left hover:bg-accent hover:text-accent-foreground"
    @click="router.push(to)"
  >
    <Icon :name="icon" class="size-5 shrink-0 text-primary" />
    <span class="flex min-w-0 flex-1 flex-col">
      <span class="font-medium">{{ title }}</span>
      <span class="truncate text-sm text-muted-foreground">
        {{ description }}
      </span>
    </span>
    <Icon
      name="lucide:chevron-right"
      class="size-4 shrink-0 text-muted-foreground"
    />
  </button>
</template>
