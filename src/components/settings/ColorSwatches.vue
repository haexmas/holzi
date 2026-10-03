<script setup lang="ts">
/**
 * A row of colour fields and a "+" for a custom colour (spec 035-appearance-and-fields, FR-012,
 * COSMIC "Akzentfarbe"): radio buttons with a mark on the chosen one, the user's own colour as one
 * more field, and a popover with the system colour picker and a hex field. A half-typed hex value
 * changes nothing; the last valid colour stays.
 */
import {
  presetsFor,
  type ColorChoice,
  type Control,
} from '~/lib/appearance/schema'
import { isHexColor } from '~/lib/appearance/oklch'

const props = defineProps<{
  control: Control
  modelValue: ColorChoice
  disabled?: boolean
  /** Id of the row title that names the group. */
  labelledby?: string
}>()
const emit = defineEmits<{ select: [choice: ColorChoice] }>()

const { t } = useI18n()
const fieldLabels = useFieldLabels()

const presets = computed(() => presetsFor(props.control))
const custom = computed(() =>
  'custom' in props.modelValue ? props.modelValue.custom : null,
)

/** The colour a field shows: a sample of the accent, or a light sample of the tint. */
function swatchColor(preset: { h: number; c: number }): string {
  return props.control === 'accent'
    ? `oklch(0.65 ${preset.c} ${preset.h})`
    : `oklch(0.85 ${preset.c * 3} ${preset.h})`
}

type Entry = { key: string; choice: ColorChoice; color: string; label: string }
const entries = computed<Entry[]>(() => {
  const list: Entry[] = presets.value.map((preset) => ({
    key: preset.id,
    choice: { preset: preset.id },
    color: swatchColor(preset),
    label: t(`settings.appearance.preset.${preset.id}`),
  }))
  if (custom.value) {
    list.push({
      key: 'custom',
      choice: { custom: custom.value },
      color: custom.value,
      label: t('settings.appearance.custom'),
    })
  }
  return list
})
/** The custom field exists only while the choice is a custom colour, so it is the chosen one. */
const isChecked = (entry: Entry) =>
  'custom' in entry.choice ||
  ('preset' in props.modelValue && props.modelValue.preset === entry.key)
const selectedIndex = computed(() =>
  Math.max(0, entries.value.findIndex(isChecked)),
)

const buttons = useTemplateRef<HTMLButtonElement[]>('buttons')
function move(index: number, step: number) {
  const next = (index + step + entries.value.length) % entries.value.length
  emit('select', entries.value[next]!.choice)
  void nextTick(() => buttons.value?.[next]?.focus())
}

const pickerOpen = ref(false)
const hex = ref(custom.value ?? '#336699')
watch(custom, (value) => {
  if (value) hex.value = value
})
function pick(value: string) {
  hex.value = value
  if (isHexColor(value)) emit('select', { custom: value.toLowerCase() })
}
</script>

<template>
  <div
    class="flex flex-wrap items-center gap-2"
    role="radiogroup"
    :aria-labelledby="labelledby"
    :aria-label="labelledby ? undefined : t('settings.appearance.swatches')"
  >
    <button
      v-for="(entry, index) in entries"
      :key="entry.key"
      ref="buttons"
      type="button"
      role="radio"
      :aria-checked="isChecked(entry)"
      :aria-label="entry.label"
      :title="entry.label"
      :tabindex="index === selectedIndex ? 0 : -1"
      :disabled="disabled"
      class="relative flex size-8 items-center justify-center rounded-lg border border-foreground/20 transition-shadow focus-visible:ring-[3px] focus-visible:ring-primary/50 focus-visible:outline-none disabled:opacity-50"
      :class="
        isChecked(entry)
          ? 'ring-2 ring-primary ring-offset-2 ring-offset-muted'
          : ''
      "
      :style="{ backgroundColor: entry.color }"
      :data-testid="`appearance-swatch-${control}-${entry.key}`"
      @click="emit('select', entry.choice)"
      @keydown.right.prevent="move(index, 1)"
      @keydown.down.prevent="move(index, 1)"
      @keydown.left.prevent="move(index, -1)"
      @keydown.up.prevent="move(index, -1)"
    >
      <Icon
        v-if="isChecked(entry)"
        name="lucide:check"
        class="size-4 mix-blend-difference"
        style="color: white"
      />
    </button>

    <ShadcnPopover v-model:open="pickerOpen">
      <ShadcnPopoverTrigger as-child>
        <button
          type="button"
          :aria-label="t('settings.appearance.addCustom')"
          :title="t('settings.appearance.addCustom')"
          :disabled="disabled"
          class="flex size-8 items-center justify-center rounded-lg border border-foreground/20 bg-background hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-primary/50 focus-visible:outline-none disabled:opacity-50"
          :data-testid="`appearance-add-${control}`"
        >
          <Icon name="lucide:plus" class="size-4" />
        </button>
      </ShadcnPopoverTrigger>
      <ShadcnPopoverContent class="flex w-64 flex-col gap-3">
        <input
          type="color"
          :value="isHexColor(hex) ? hex : '#336699'"
          class="h-24 w-full cursor-pointer rounded-md border border-input bg-transparent"
          :aria-label="t('settings.appearance.addCustom')"
          :data-testid="`appearance-picker-${control}`"
          @input="pick(($event.target as HTMLInputElement).value)"
        />
        <UiInput
          :model-value="hex"
          :label="t('settings.appearance.hex')"
          :labels="fieldLabels.input.value"
          label-bg="var(--popover)"
          spellcheck="false"
          autocomplete="off"
          :data-testid="`appearance-hex-${control}`"
          @update:model-value="pick(String($event ?? ''))"
        />
      </ShadcnPopoverContent>
    </ShadcnPopover>
  </div>
</template>
