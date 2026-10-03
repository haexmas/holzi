<script setup lang="ts">
/**
 * One colour row of "Darstellung" (spec 035-appearance-and-fields, FR-012, FR-013, FR-017): a title,
 * the colour fields, and under it the note "angepasst" with its reason when the choice had to change
 * to stay readable. The choice is saved as soon as it is made (no save button) through
 * `settings.appearance.set`; the row says nothing on success, the view shows one status line.
 */
import type { ColorChoice, Control } from '~/lib/appearance/schema'

const props = defineProps<{
  control: Control
  /** Language key suffix under `settings.appearance`: `accent`, `window`, `container`, … */
  titleKey: string
  /** Language key of the one line under the title, if the row has one. */
  hintKey?: string
}>()
const emit = defineEmits<{ saved: []; failed: [message: string] }>()

const { t } = useI18n()
const { errString } = useErrorString()
const appearance = useAppearance()
const setAppearance = useActionOrThrow('settings.appearance.set')
const busy = ref(false)

const choice = computed<ColorChoice>(
  () => appearance.appearance.value[props.control],
)
const reasons = computed(() => [
  ...new Set(
    appearance.adjustments.value
      .filter((adjustment) => adjustment.control === props.control)
      .map((adjustment) => adjustment.reason),
  ),
])
const titleId = computed(() => `appearance-${props.control}-title`)

async function selectAsync(next: ColorChoice) {
  if (busy.value) return
  busy.value = true
  try {
    await setAppearance({ [props.control]: next })
    emit('saved')
  } catch (error: unknown) {
    emit('failed', errString(error))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <SettingsRow
    :title="t(`settings.appearance.${titleKey}`)"
    :description="hintKey ? t(`settings.appearance.${hintKey}`) : undefined"
  >
    <template #title>
      <span :id="titleId">{{ t(`settings.appearance.${titleKey}`) }}</span>
    </template>
    <SettingsColorSwatches
      :control="control"
      :model-value="choice"
      :disabled="busy"
      :labelledby="titleId"
      @select="selectAsync"
    />
    <template v-if="reasons.length" #below>
      <p
        class="text-xs text-muted-foreground"
        data-testid="appearance-adjusted"
      >
        <span class="font-medium"
          >{{ t('settings.appearance.adjusted') }}:</span
        >
        {{
          reasons
            .map((reason) => t(`settings.appearance.reasons.${reason}`))
            .join(' ')
        }}
      </p>
    </template>
  </SettingsRow>
</template>
