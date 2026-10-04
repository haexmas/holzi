<script setup lang="ts">
/**
 * The tab Extra of an entry in view mode (spec 036, FR-001, FR-005): the custom fields, the
 * attachments and the passkeys. The custom values stay masked until asked; their rows carry
 * `data-no-swipe` so a swipe that starts on a value does not change the tab (FR-003).
 */
import type { CopyField } from '@bindings/CopyField'
import type { ItemDetail } from '@bindings/ItemDetail'

const emit = defineEmits<{
  copy: [field: CopyField, label: string]
  changed: []
}>()

const props = defineProps<{
  itemId: string
  detail: ItemDetail
}>()

const { t } = useI18n()
const { revealAsync } = usePasswords()

/** The marks of a custom field that holds references (spec 036). */
function marksOf(id: string) {
  return (
    props.detail.references.keyValues.find((entry) => entry.id === id)?.marks ??
    []
  )
}
</script>

<template>
  <div class="flex flex-col gap-4" data-testid="entry-view-extra">
    <SettingsGroup
      v-if="detail.keyValues.length"
      :label="t('passwords.fields.custom')"
    >
      <SettingsRow
        v-for="field in detail.keyValues"
        :key="field.id"
        :title="field.key ?? ''"
        data-no-swipe
      >
        <template v-if="marksOf(field.id).length" #below>
          <PasswordsReferenceValue
            :marks="marksOf(field.id)"
            :text="null"
            kind="keyValue"
          />
        </template>
        <PasswordsMaskedValue
          :fetch="
            async () =>
              (
                await revealAsync(itemId, {
                  kind: 'keyValue',
                  id: field.id,
                })
              ).value
          "
          :identity="`${itemId}:${field.id}`"
          kind="keyValue"
          :present="field.hasValue"
          :label="field.key ?? ''"
        />
        <UiButton
          v-if="field.hasValue"
          variant="ghost"
          size="icon"
          :aria-label="t('passwords.copy', { field: field.key ?? '' })"
          @click="
            emit('copy', { kind: 'keyValue', id: field.id }, field.key ?? '')
          "
        >
          <Icon name="lucide:copy" class="size-4" />
        </UiButton>
      </SettingsRow>
    </SettingsGroup>

    <PasswordsAttachments
      :item-id="itemId"
      :attachments="detail.attachments"
      @changed="emit('changed')"
    />

    <PasswordsPasskeys
      :item-id="itemId"
      :passkeys="detail.passkeys"
      @changed="emit('changed')"
    />
  </div>
</template>
