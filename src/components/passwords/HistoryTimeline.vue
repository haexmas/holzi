<script setup lang="ts">
/**
 * The selector for the states of an entry (spec 036, US2, FR-007): the newest state is first and
 * each option shows its exact timestamp and changed fields. Selection stays in this existing
 * component so the parent keeps the same event contract while the presentation remains compact.
 */
import type { SnapshotHeader } from '@bindings/SnapshotHeader'

const props = defineProps<{
  states: readonly SnapshotHeader[]
  selectedId: string | null
}>()
const emit = defineEmits<{ select: [id: string] }>()

const { t, d } = useI18n()

function exact(stamp: string | null): string {
  if (!stamp) return '–'
  const date = new Date(stamp)
  return Number.isNaN(date.getTime())
    ? stamp
    : d(date, { dateStyle: 'medium', timeStyle: 'medium' })
}

function changedLabel(names: readonly string[]): string {
  return names.map((name) => t(`passwords.history.fields.${name}`)).join(', ')
}

const options = computed(() =>
  props.states.map((state) => ({
    value: state.id,
    label: state.changedFields.length
      ? `${exact(state.modifiedAt)} · ${changedLabel(state.changedFields)}`
      : exact(state.modifiedAt),
  })),
)

function selectValue(value: string | null | undefined) {
  if (value) emit('select', value)
}
</script>

<template>
  <div class="pt-2">
    <UiSelect
      id="passwords-history-state"
      :model-value="selectedId ?? options[0]?.value ?? ''"
      :options="options"
      :label="t('passwords.history.states')"
      data-testid="passwords-history-select"
      @update:model-value="selectValue"
    />
  </div>
</template>
