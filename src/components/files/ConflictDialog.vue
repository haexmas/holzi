<script setup lang="ts">
/**
 * A name a transfer finds taken at its target (spec 044 FR-021): replace, keep both (the new one
 * gets a number) or skip, optionally for every further name of this transfer. Closing the dialog
 * cancels the transfer.
 */
import type { ConflictChoice } from '@bindings/ConflictChoice'

const props = defineProps<{ open: boolean; name: string }>()
const emit = defineEmits<{
  answer: [choice: ConflictChoice, forAll: boolean]
  cancel: []
}>()

const { t } = useI18n()
const forAll = ref(false)
watch(
  () => props.open,
  (open) => {
    if (open) forAll.value = false
  },
)
</script>

<template>
  <ShadcnAlertDialog
    :open="open"
    @update:open="(value: boolean) => !value && emit('cancel')"
  >
    <ShadcnAlertDialogContent data-testid="files-conflict">
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>{{
          t('files.conflict.title')
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription class="break-all">
          {{ t('files.conflict.body', { name }) }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <label class="flex items-center gap-2 text-sm">
        <ShadcnCheckbox v-model="forAll" data-testid="files-conflict-for-all" />
        {{ t('files.conflict.forAll') }}
      </label>
      <ShadcnAlertDialogFooter class="flex-wrap gap-2">
        <ShadcnAlertDialogCancel data-testid="files-conflict-cancel">
          {{ t('files.cancel') }}
        </ShadcnAlertDialogCancel>
        <UiButton
          variant="outline"
          data-testid="files-conflict-skip"
          @click="emit('answer', 'skip', forAll)"
        >
          {{ t('files.conflict.skip') }}
        </UiButton>
        <UiButton
          variant="outline"
          data-testid="files-conflict-keep-both"
          @click="emit('answer', 'keepBoth', forAll)"
        >
          {{ t('files.conflict.keepBoth') }}
        </UiButton>
        <UiButton
          data-testid="files-conflict-replace"
          @click="emit('answer', 'replace', forAll)"
        >
          {{ t('files.conflict.replace') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
