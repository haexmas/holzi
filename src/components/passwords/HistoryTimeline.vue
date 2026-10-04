<script setup lang="ts">
/**
 * The timeline of the states of an entry (spec 036, US2, FR-007): one dot per state on a line, the
 * newest on top. With room each row names the time ("vor 3 Tagen", the exact time as a hint) and
 * the fields that changed against the state before; a very narrow window shows the dots only (the
 * chosen state names itself next to the timeline). The list is a radio group: arrow keys move the
 * choice.
 */
import type { SnapshotHeader } from '@bindings/SnapshotHeader'
import { relativeTime } from '~/lib/passwords/format'

const props = defineProps<{
  states: readonly SnapshotHeader[]
  selectedId: string | null
}>()
const emit = defineEmits<{ select: [id: string] }>()

const { t, d, locale } = useI18n()

function selectWithArrow(event: KeyboardEvent, index: number) {
  const direction =
    event.key === 'ArrowDown' || event.key === 'ArrowRight'
      ? 1
      : event.key === 'ArrowUp' || event.key === 'ArrowLeft'
        ? -1
        : 0
  if (direction === 0 || props.states.length === 0) return
  event.preventDefault()
  const nextIndex =
    (index + direction + props.states.length) % props.states.length
  const next = props.states[nextIndex]
  if (!next) return
  emit('select', next.id)
  void nextTick(() => {
    timeline.value
      ?.querySelectorAll<HTMLButtonElement>('[role="radio"]')
      [nextIndex]?.focus()
  })
}

const timeline = ref<HTMLOListElement | null>(null)

function exact(stamp: string | null): string {
  if (!stamp) return '–'
  const date = new Date(stamp)
  return Number.isNaN(date.getTime())
    ? stamp
    : d(date, { dateStyle: 'medium', timeStyle: 'medium' })
}

function ago(stamp: string | null): string {
  if (!stamp) return '–'
  const date = new Date(stamp)
  return Number.isNaN(date.getTime())
    ? stamp
    : relativeTime(date, new Date(), locale.value)
}

function changedLabel(names: readonly string[]): string {
  return names.map((name) => t(`passwords.history.fields.${name}`)).join(', ')
}
</script>

<template>
  <ol
    ref="timeline"
    class="flex gap-2 overflow-x-auto @md:flex-col @md:gap-0 @md:overflow-visible"
    role="radiogroup"
    :aria-label="t('passwords.history.states')"
    data-no-swipe
    data-testid="passwords-history-timeline"
  >
    <li
      v-for="(state, index) in states"
      :key="state.id"
      class="relative flex shrink-0 @md:shrink"
    >
      <span
        v-if="index < states.length - 1"
        class="absolute top-6 bottom-0 left-[1.1rem] hidden w-px bg-border @md:block"
        aria-hidden="true"
      />
      <button
        type="button"
        role="radio"
        :aria-checked="selectedId === state.id"
        :title="exact(state.modifiedAt)"
        class="flex w-full min-w-0 items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
        :class="selectedId === state.id ? 'bg-accent' : ''"
        :tabindex="
          selectedId === state.id || (!selectedId && index === 0) ? 0 : -1
        "
        :data-testid="`passwords-history-state-${state.id}`"
        @click="emit('select', state.id)"
        @keydown="selectWithArrow($event, index)"
      >
        <span
          class="relative z-10 mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full border bg-background"
          :class="
            selectedId === state.id
              ? 'border-primary text-primary'
              : 'border-border'
          "
        >
          <Icon name="lucide:clock" class="size-3" />
        </span>
        <span class="hidden min-w-0 flex-1 flex-col @md:flex">
          <span class="truncate text-sm font-medium">{{
            ago(state.modifiedAt)
          }}</span>
          <span
            v-if="state.changedFields.length"
            class="truncate text-xs text-muted-foreground"
            >{{ changedLabel(state.changedFields) }}</span
          >
        </span>
        <span class="sr-only @md:hidden">{{ ago(state.modifiedAt) }}</span>
      </button>
    </li>
  </ol>
</template>
