<script setup lang="ts">
/**
 * A boxed list like in the COSMIC and GNOME settings (spec 023-settings-app, FR-002): rows of
 * `SettingsRow`/`SettingsOptionRow` on a rounded surface, divided by thin lines. `label` is a
 * short group name above the box, only where a view has more than one group.
 */
defineProps<{
  label?: string
}>()
</script>

<template>
  <section class="flex flex-col gap-2">
    <h2 v-if="label" class="px-1 text-sm font-semibold">{{ label }}</h2>
    <ul
      class="flex flex-col divide-y divide-background overflow-hidden rounded-xl bg-muted"
    >
      <slot />
    </ul>
  </section>
</template>
