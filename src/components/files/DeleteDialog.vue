<script setup lang="ts">
/**
 * The question before deleting for good (spec 044 FR-023): on Android, where there is no trash,
 * it names how many entries go. On desktops deleting moves to the trash and asks nothing.
 */
defineProps<{ count: number }>()
const emit = defineEmits<{ confirm: [] }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()

function confirm() {
  open.value = false
  emit('confirm')
}
</script>

<template>
  <ShadcnAlertDialog
    :open="open"
    @update:open="(value: boolean) => (open = value)"
  >
    <ShadcnAlertDialogContent>
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>{{
          t('files.delete.title')
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription data-testid="files-delete-count">
          {{ t('files.delete.body', { count }, count) }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel>{{
          t('files.cancel')
        }}</ShadcnAlertDialogCancel>
        <UiButton
          variant="destructive"
          data-testid="files-delete-confirm"
          @click="confirm"
        >
          {{ t('files.delete.confirm') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
