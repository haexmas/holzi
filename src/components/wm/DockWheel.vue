<script setup lang="ts">
/**
 * The dock as a wheel (spec 045, US5): one round button that fans the entries out over a quarter
 * circle at a corner or a half circle at the middle of an edge, on further rings when they do not
 * fit (`wheelLayout`, FR-027, FR-028). It folds when an entry is activated, on Escape and on a
 * click outside (FR-029), but stays open while an entry's menu or chooser is (they live outside
 * it). A menu of entries (FR-042): Enter or Space opens it, the arrow keys follow the entries,
 * Enter activates. The button carries a dot while any app wants attention.
 */
import { computed, inject, nextTick, provide, ref, useTemplateRef } from 'vue'
import { DOCK_HOLD } from '~/composables/useDock'
import {
  dockItemKey,
  wheelLayout,
  type DockAlign,
  type DockEdge,
  type DockEntry,
} from '~/lib/wm/dock'

const props = defineProps<{
  entries: DockEntry[]
  edge: DockEdge
  align: DockAlign
}>()

const wm = useWindowManagerStore()
const { t } = useI18n()

const root = useTemplateRef<HTMLElement>('root')
const toggle = useTemplateRef<HTMLButtonElement>('toggle')
const open = ref(false)
const held = ref(0)
const focusIndex = ref(0)

const parentHold = inject(DOCK_HOLD, () => {})
provide(DOCK_HOLD, (isOpen: boolean) => {
  held.value = Math.max(0, held.value + (isOpen ? 1 : -1))
  parentHold(isOpen)
  // The chooser of an entry closed after a choice: the wheel's job is done too.
  if (!isOpen && held.value === 0) close()
})

const offsets = computed(() =>
  wheelLayout(props.entries.length, props.edge, props.align),
)

const attention = computed(() =>
  props.entries.some(
    (entry) => entry.kind === 'app' && wm.appHasAttention(entry.appId),
  ),
)

function items(): HTMLElement[] {
  return [
    ...(root.value?.querySelectorAll<HTMLElement>('[data-dock-item]') ?? []),
  ]
}

async function show() {
  open.value = true
  focusIndex.value = 0
  await nextTick()
  items()[0]?.focus()
}

function close() {
  open.value = false
}

function onToggle() {
  if (open.value) close()
  else void show()
}

/** An activated entry folds the wheel, unless it opened its chooser, which then holds it. */
function onItemClick() {
  void nextTick(() => {
    if (held.value === 0) close()
  })
}

onClickOutside(root, () => {
  if (held.value === 0) close()
})

function onKeydown(event: KeyboardEvent) {
  if (!open.value) return
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    toggle.value?.focus()
    return
  }
  const all = items()
  if (all.length === 0) return
  let next: number
  if (event.key === 'ArrowRight' || event.key === 'ArrowDown')
    next = (focusIndex.value + 1) % all.length
  else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp')
    next = (focusIndex.value - 1 + all.length) % all.length
  else return
  event.preventDefault()
  focusIndex.value = next
  all[next]?.focus()
}
</script>

<template>
  <div ref="root" class="relative h-14 w-14" @keydown="onKeydown">
    <div
      role="menu"
      :aria-label="t('wm.dock.label')"
      :aria-hidden="!open"
      class="absolute inset-0"
    >
      <div
        v-for="(entry, index) in entries"
        :key="dockItemKey(entry)"
        role="none"
        class="absolute left-1/2 top-1/2 rounded-full bg-background shadow-lg ring-1 ring-border transition-[transform,opacity] duration-200 motion-reduce:transition-none"
        :class="open ? 'opacity-100' : 'pointer-events-none opacity-0'"
        :style="{
          transform: open
            ? `translate(-50%, -50%) translate(${offsets[index]?.x ?? 0}px, ${offsets[index]?.y ?? 0}px)`
            : 'translate(-50%, -50%) scale(0.5)',
        }"
        @click="onItemClick"
      >
        <WmDockItem :entry="entry" :tabbable="open && index === focusIndex" />
      </div>
    </div>
    <ShadcnContextMenu @update:open="parentHold">
      <ShadcnContextMenuTrigger as-child>
        <button
          ref="toggle"
          type="button"
          class="relative flex h-14 w-14 items-center justify-center rounded-full bg-foreground text-background shadow-lg hover:opacity-90"
          data-testid="dock-wheel-toggle"
          aria-haspopup="menu"
          :aria-expanded="open"
          :aria-label="
            open ? t('wm.dock.wheel.close') : t('wm.dock.wheel.open')
          "
          :title="open ? t('wm.dock.wheel.close') : t('wm.dock.wheel.open')"
          @click="onToggle"
        >
          <Icon
            :name="open ? 'lucide:x' : 'lucide:circle-dot'"
            class="h-6 w-6"
            :aria-hidden="true"
          />
          <span
            v-if="attention && !open"
            class="absolute right-1 top-1 h-2 w-2 rounded-full bg-warning"
            :aria-label="t('wm.attention')"
          />
        </button>
      </ShadcnContextMenuTrigger>
      <ShadcnContextMenuContent class="min-w-48" data-testid="dock-menu">
        <WmDockPlacementMenu />
      </ShadcnContextMenuContent>
    </ShadcnContextMenu>
  </div>
</template>
