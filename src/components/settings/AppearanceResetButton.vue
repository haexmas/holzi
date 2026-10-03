<script setup lang="ts">
/**
 * "Auf Standard zurücksetzen" (spec 035-appearance-and-fields, FR-011, FR-017): all values of the
 * appearance go back to the defaults after a confirmation; the colour scheme stays.
 */
const emit = defineEmits<{ done: []; failed: [message: string] }>()

const { t } = useI18n()
const { errString } = useErrorString()
const resetAction = useActionOrThrow('settings.appearance.reset')
const open = ref(false)
const busy = ref(false)

async function resetAsync() {
  busy.value = true
  try {
    await resetAction()
    open.value = false
    emit('done')
  } catch (error: unknown) {
    open.value = false
    emit('failed', errString(error))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <ShadcnAlertDialog v-model:open="open">
    <UiButton
      variant="outline"
      size="sm"
      data-testid="appearance-reset"
      @click="open = true"
    >
      {{ t('settings.appearance.reset') }}
    </UiButton>
    <ShadcnAlertDialogContent>
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>{{
          t('settings.appearance.resetTitle')
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription>{{
          t('settings.appearance.resetBody')
        }}</ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel>{{
          t('settings.appearance.cancel')
        }}</ShadcnAlertDialogCancel>
        <UiButton
          :loading="busy"
          data-testid="appearance-reset-confirm"
          @click="resetAsync"
        >
          {{ t('settings.appearance.resetConfirm') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
