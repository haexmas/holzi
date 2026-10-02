<script setup lang="ts">
/**
 * What the editor offers when a save finds the entry changed or deleted meanwhile (spec 034,
 * research R15): nothing is overwritten silently. The user keeps their changes as a new entry, or
 * takes over the current state (and loses the draft), or stays in the editor.
 */
defineProps<{
  open: boolean
  /** `changed` or `deleted`. */
  reason: 'changed' | 'deleted'
}>()

const emit = defineEmits<{
  saveAsNew: []
  takeCurrent: []
  close: []
}>()

const { t } = useI18n()
</script>

<template>
  <ShadcnAlertDialog
    :open="open"
    @update:open="(value: boolean) => !value && emit('close')"
  >
    <ShadcnAlertDialogContent>
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>
          {{ t('passwords.conflict.title') }}
        </ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription>
          {{
            reason === 'deleted'
              ? t('passwords.conflict.deleted')
              : t('passwords.conflict.changed')
          }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter class="flex-wrap gap-2">
        <ShadcnAlertDialogCancel data-testid="passwords-conflict-stay">
          {{ t('passwords.conflict.stay') }}
        </ShadcnAlertDialogCancel>
        <UiButton
          v-if="reason !== 'deleted'"
          variant="outline"
          data-testid="passwords-conflict-take"
          @click="emit('takeCurrent')"
        >
          {{ t('passwords.conflict.takeCurrent') }}
        </UiButton>
        <UiButton
          data-testid="passwords-conflict-save-new"
          @click="emit('saveAsNew')"
        >
          {{ t('passwords.conflict.saveAsNew') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
