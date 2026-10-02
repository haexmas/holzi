<script setup lang="ts">
/**
 * A secret that stays hidden until the user asks (spec 034, FR-005, US1 scenario 2): held with
 * the mouse, or tapped on a touch screen, or toggled with the keyboard. The value is fetched for
 * that moment, lives only in this component and ends when the pointer lets go (mouse), when the
 * entry or place is left, or when the entry changes. It is never written to the store, the title or
 * the history.
 */
import type { SecretField } from '@bindings/SecretField'

const props = defineProps<{
  itemId: string
  field: SecretField
  /** Whether the secret has a value at all; without one nothing can be revealed. */
  present: boolean
  label: string
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const { revealAsync } = usePasswords()

const value = ref<string | null>(null)
const error = ref<string | null>(null)
let token = 0

function hide() {
  token += 1
  value.value = null
}

async function showAsync() {
  const mine = ++token
  try {
    const revealed = await revealAsync(props.itemId, props.field)
    // A later hide or show outran this one.
    if (mine === token) value.value = revealed.value
    error.value = null
  } catch (cause) {
    error.value = errString(cause)
  }
}

function toggle() {
  if (value.value === null) void showAsync()
  else hide()
}

function onPointerDown(event: PointerEvent) {
  if (event.pointerType === 'touch') return
  void showAsync()
}

function onPointerEnd(event: PointerEvent) {
  if (event.pointerType !== 'touch') hide()
}

function onClick(event: MouseEvent) {
  // Mouse and pen hold; a tap on a touch screen and the keyboard (detail 0) toggle.
  const pointerHold =
    event.detail > 0 && (event as PointerEvent).pointerType !== 'touch'
  if (!pointerHold) toggle()
}

watch(() => [props.itemId, props.field], hide, { deep: true })
onBeforeUnmount(hide)
</script>

<template>
  <span class="flex min-w-0 items-center gap-2">
    <span
      class="min-w-0 flex-1 truncate font-mono text-sm"
      :class="value === null ? 'text-muted-foreground' : ''"
      :data-testid="`passwords-value-${field.kind}`"
    >
      <template v-if="!present">{{ t('passwords.noValue') }}</template>
      <template v-else-if="value === null">••••••••</template>
      <template v-else>{{ value }}</template>
    </span>
    <UiButton
      v-if="present"
      variant="ghost"
      size="icon"
      class="shrink-0"
      :aria-label="t('passwords.reveal', { field: label })"
      :aria-pressed="value !== null"
      :tooltip="t('passwords.revealHint')"
      :data-testid="`passwords-reveal-${field.kind}`"
      @pointerdown="onPointerDown"
      @pointerup="onPointerEnd"
      @pointerleave="onPointerEnd"
      @pointercancel="onPointerEnd"
      @click="onClick"
    >
      <Icon
        :name="value === null ? 'lucide:eye' : 'lucide:eye-off'"
        class="size-4"
      />
    </UiButton>
    <span v-if="error" class="text-xs text-destructive" role="alert">{{
      error
    }}</span>
  </span>
</template>
