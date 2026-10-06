<script setup lang="ts">
/**
 * "Verweis einfügen" of an editor field (spec 036, US7, research R11): an icon button for the
 * append slot of the field that opens the picker and appends the chosen placeholder to the text
 * (no cursor position). The marks below the field are `ReferenceField.vue`.
 */
const props = defineProps<{
  text: string
  /** The entry being edited, left out of the picker; `null` for a new one. */
  itemId: string | null
  /** Part of the test ids. */
  kind: string
}>()

const emit = defineEmits<{
  'update:text': [value: string]
}>()

const { t } = useI18n()
const pickerOpen = ref(false)
</script>

<template>
  <UiButton
    type="button"
    variant="ghost"
    size="icon"
    class="shrink-0 shadow-none"
    :aria-label="t('passwords.references.insert')"
    :tooltip="t('passwords.references.insert')"
    :data-testid="`passwords-reference-insert-${kind}`"
    @click.prevent="pickerOpen = true"
  >
    <Icon name="lucide:link" class="size-4" />
  </UiButton>
  <PasswordsReferencePicker
    v-model:open="pickerOpen"
    :item-id="itemId"
    @pick="emit('update:text', props.text + $event)"
  />
</template>
