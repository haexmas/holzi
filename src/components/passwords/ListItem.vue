<script setup lang="ts">
/**
 * One entry in the list (spec 034, FR-001, FR-008, US1): icon, title (or the placeholder text of the
 * window for an entry without one), username, tag chips, a badge when it has expired and markers
 * for a TOTP and for passkeys. It holds a header only, never a secret.
 */
import type { ItemHeader } from '@bindings/ItemHeader'
import { DEFAULT_ENTRY_ICON } from '~/lib/passwords/icons'
import { draggedIds, ITEMS_MIME, itemsPayload } from '~/lib/passwords/dnd'
import { displayTitle, isExpired, localDay } from '~/lib/passwords/format'

const props = defineProps<{
  header: ItemHeader
  /** Whether the entry is part of the selection. */
  selected?: boolean
  /** Whether a selection is going on (a plain click then toggles instead of opening). */
  selecting?: boolean
}>()

const emit = defineEmits<{
  /** A click with its modifier keys; the list decides between open, toggle and range. */
  activate: [event: MouseEvent]
  /** A long press on a touch screen starts or extends the selection. */
  longPress: []
}>()

const { t } = useI18n()

const title = computed(() => displayTitle(props.header.title))
const expired = computed(() =>
  isExpired(props.header.expiresAt, localDay(new Date())),
)

const selection = usePasswordsSelectionStore()

let pressTimer: ReturnType<typeof setTimeout> | null = null
let pressed = false

function startPress(event: PointerEvent) {
  if (event.pointerType !== 'touch') return
  pressed = false
  pressTimer = setTimeout(() => {
    pressed = true
    emit('longPress')
  }, 500)
}

function endPress() {
  if (pressTimer !== null) clearTimeout(pressTimer)
  pressTimer = null
}

function onClick(event: MouseEvent) {
  // The click that ends a long press is not an activation.
  if (pressed) {
    pressed = false
    return
  }
  emit('activate', event)
}

function onDragStart(event: DragEvent) {
  const ids = draggedIds(props.header.id, selection.ids)
  event.dataTransfer?.setData(ITEMS_MIME, itemsPayload(ids))
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}
</script>

<template>
  <li>
    <button
      type="button"
      class="flex min-h-14 w-full items-center gap-4 px-4 py-3 text-left hover:bg-foreground/5"
      :class="selected ? 'bg-primary/10' : ''"
      :aria-pressed="selecting ? selected : undefined"
      draggable="true"
      :data-testid="`passwords-entry-${header.id}`"
      @click="onClick"
      @dragstart="onDragStart"
      @pointerdown="startPress"
      @pointerup="endPress"
      @pointerleave="endPress"
      @pointercancel="endPress"
    >
      <ShadcnCheckbox
        v-if="selecting"
        :model-value="selected"
        class="pointer-events-none shrink-0"
        tabindex="-1"
        aria-hidden="true"
      />
      <span
        class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-background"
        :style="header.color ? { color: header.color } : undefined"
        aria-hidden="true"
      >
        <span class="size-5">
          <PasswordsEntryIcon
            :value="header.icon"
            :fallback="DEFAULT_ENTRY_ICON"
          />
        </span>
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <span
          class="truncate"
          :class="title === null ? 'text-muted-foreground italic' : ''"
        >
          {{ title ?? t('passwords.untitled') }}
        </span>
        <span
          v-if="header.username"
          class="truncate text-sm text-muted-foreground"
        >
          {{ header.username }}
        </span>
      </span>
      <span class="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
        <ShadcnBadge v-if="expired" variant="destructive">
          {{ t('passwords.expired') }}
        </ShadcnBadge>
        <span
          v-if="header.hasTotp"
          class="text-muted-foreground"
          :title="t('passwords.markers.totp')"
        >
          <Icon name="lucide:timer" class="size-4" />
          <span class="sr-only">{{ t('passwords.markers.totp') }}</span>
        </span>
        <span
          v-if="header.passkeyCount > 0"
          class="text-muted-foreground"
          :title="t('passwords.markers.passkey')"
        >
          <Icon name="lucide:fingerprint" class="size-4" />
          <span class="sr-only">{{ t('passwords.markers.passkey') }}</span>
        </span>
        <ShadcnBadge
          v-for="tag in header.tags"
          :key="tag.id"
          variant="secondary"
          :style="tag.color ? { color: tag.color } : undefined"
        >
          {{ tag.name }}
        </ShadcnBadge>
      </span>
    </button>
  </li>
</template>
