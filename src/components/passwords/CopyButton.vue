<script setup lang="ts">
/** The copy button of a field (`usePasswordsCopy` does the copying), in a row or in the append slot
 * of an input. */
defineProps<{
  /** The field's name, for the label and the tooltip. */
  label: string
}>()
const emit = defineEmits<{ copy: [] }>()

const { t } = useI18n()
</script>

<template>
  <UiButton
    type="button"
    variant="ghost"
    size="icon"
    class="shrink-0 shadow-none"
    :aria-label="t('passwords.copy', { field: label })"
    :tooltip="t('passwords.copy', { field: label })"
    @click.prevent="emit('copy')"
  >
    <Icon name="lucide:copy" class="size-4" />
  </UiButton>
</template>
