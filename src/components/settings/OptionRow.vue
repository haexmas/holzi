<script setup lang="ts">
/**
 * A choice inside a `SettingsGroup` (spec 023-settings-app, FR-002, FR-021): the whole row is the
 * label of its radio button or checkbox, so a click anywhere on it chooses. Saving on the change
 * is the caller's (`change`). Attributes such as `data-testid` go to the input.
 */
defineOptions({ inheritAttrs: false })

defineProps<{
  type: 'radio' | 'checkbox'
  name?: string
  value?: string
  checked: boolean
  disabled?: boolean
  title: string
  description?: string
}>()

const emit = defineEmits<{
  change: [checked: boolean]
}>()
</script>

<template>
  <li>
    <label
      class="flex min-h-14 cursor-pointer items-center gap-4 px-4 py-3 hover:bg-foreground/5 has-disabled:cursor-default has-disabled:opacity-60"
    >
      <input
        :type="type"
        :name="name"
        :value="value"
        :checked="checked"
        :disabled="disabled"
        class="size-4 shrink-0 accent-primary"
        v-bind="$attrs"
        @change="emit('change', ($event.target as HTMLInputElement).checked)"
      />
      <span class="flex min-w-0 flex-1 flex-col">
        <span>{{ title }}</span>
        <span v-if="description" class="text-sm text-muted-foreground">
          {{ description }}
        </span>
      </span>
    </label>
  </li>
</template>
