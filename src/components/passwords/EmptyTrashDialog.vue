<script setup lang="ts">
/**
 * The confirmation before the trash is emptied (spec 034, US4, FR-015): it names the number of
 * entries and folders that go for good, and that this cannot be undone.
 */
import { toast } from 'vue-sonner'

defineProps<{
  entries: number
  folders: number
  /** The entries in the trash, for the references other entries hold on them (spec 036). */
  itemIds: readonly string[]
}>()

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const { errString } = useErrorString()
const store = usePasswordsStore()
const { emptyTrashAsync } = usePasswords()
const busy = ref(false)
const inlineReferences = ref(true)

async function confirmAsync() {
  busy.value = true
  try {
    await emptyTrashAsync(inlineReferences.value)
    await store.quietReloadAsync()
    open.value = false
  } catch (cause) {
    toast.error(errString(cause))
  } finally {
    busy.value = false
  }
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
          t('passwords.trash.emptyTitle')
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription data-testid="passwords-empty-trash-count">
          {{ t('passwords.trash.emptyBody', { entries, folders }) }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <PasswordsReferenceUsageNote
        v-if="open"
        v-model:inline="inlineReferences"
        :item-ids="itemIds"
      />
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel>{{
          t('passwords.cancel')
        }}</ShadcnAlertDialogCancel>
        <UiButton
          variant="destructive"
          :loading="busy"
          data-testid="passwords-empty-trash-confirm"
          @click="confirmAsync"
        >
          {{ t('passwords.trash.empty') }}
        </UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
