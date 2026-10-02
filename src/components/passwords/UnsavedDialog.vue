<script setup lang="ts">
/**
 * Asked when the user leaves the editor with unsaved changes (spec 034, edge cases): save, discard
 * or stay.
 */
defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  save: []
  discard: []
  stay: []
}>()

const { t } = useI18n()
</script>

<template>
  <ShadcnAlertDialog
    :open="open"
    @update:open="(value: boolean) => !value && emit('stay')"
  >
    <ShadcnAlertDialogContent>
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>
          {{ t('passwords.unsaved.title') }}
        </ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription>
          {{ t('passwords.unsaved.body') }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter class="flex-wrap gap-2">
        <ShadcnAlertDialogCancel data-testid="passwords-unsaved-stay">
          {{ t('passwords.unsaved.stay') }}
        </ShadcnAlertDialogCancel>
        <UiButton
          variant="outline"
          data-testid="passwords-unsaved-discard"
          @click="emit('discard')"
        >
          {{ t('passwords.unsaved.discard') }}
        </UiButton>
        <UiButton data-testid="passwords-unsaved-save" @click="emit('save')">
          {{ t('passwords.unsaved.save') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
