<script setup lang="ts">
/**
 * The custom fields of the entry editor (spec 034, FR-002): name and value per field, add and
 * remove. The value of a stored field is not loaded: it shows a placeholder until the user types a
 * new one, and an untouched field keeps its stored value (partial update, research R7).
 */
import type { KeyValueDraft } from '~/lib/passwords/draft'

const fields = defineModel<KeyValueDraft[]>({ required: true })

const { t } = useI18n()

function add() {
  fields.value = [
    ...fields.value,
    { id: null, key: '', value: '', hasStoredValue: false },
  ]
}

function remove(index: number) {
  fields.value = fields.value.filter((_, i) => i !== index)
}

function setKey(index: number, key: string) {
  fields.value = fields.value.map((field, i) =>
    i === index ? { ...field, key } : field,
  )
}

function setValue(index: number, value: string) {
  fields.value = fields.value.map((field, i) =>
    i === index ? { ...field, value } : field,
  )
}
</script>

<template>
  <div class="flex flex-col gap-2" data-testid="passwords-keyvalues">
    <div
      v-for="(field, index) in fields"
      :key="field.id ?? `new-${index}`"
      class="flex flex-wrap items-center gap-2"
    >
      <ShadcnInput
        :model-value="field.key"
        class="min-w-32 flex-1"
        :placeholder="t('passwords.editor.fieldName')"
        :aria-label="t('passwords.editor.fieldName')"
        :data-testid="`passwords-kv-key-${index}`"
        @update:model-value="setKey(index, String($event))"
      />
      <ShadcnInput
        :model-value="field.value ?? ''"
        type="password"
        autocomplete="off"
        class="min-w-32 flex-1"
        :placeholder="
          field.value === null && field.hasStoredValue
            ? t('passwords.editor.keepValue')
            : t('passwords.editor.fieldValue')
        "
        :aria-label="t('passwords.editor.fieldValue')"
        :data-testid="`passwords-kv-value-${index}`"
        @update:model-value="setValue(index, String($event))"
      />
      <UiButton
        variant="ghost"
        size="icon"
        class="shrink-0"
        :aria-label="t('passwords.editor.removeField')"
        :data-testid="`passwords-kv-remove-${index}`"
        @click="remove(index)"
      >
        <Icon name="lucide:trash-2" class="size-4" />
      </UiButton>
    </div>
    <UiButton
      variant="outline"
      size="sm"
      class="self-start"
      data-testid="passwords-kv-add"
      @click="add"
    >
      <Icon name="lucide:plus" class="size-4" />
      {{ t('passwords.editor.addField') }}
    </UiButton>
  </div>
</template>
