<script setup lang="ts">
/**
 * The dock of the desktop (spec 045, FR-001): the stored entries plus the running apps
 * (`resolveDockEntries`) at the effective placement (`effectivePlacement`: the user's choice, or
 * the bottom in compact mode, FR-031). Reserving space it is a flex sibling of the window area, so
 * `wm/Desktop.vue` measures what is left (FR-024); floating and hiding it lies over the windows.
 * Hiding, it comes in when the pointer reaches its edge and leaves 400 ms after the pointer, unless
 * focus is inside or one of its menus is open (FR-025). Right click on its free area sets edge,
 * alignment, style and mode (FR-018). As a wheel it always lies over the windows and keeps no space
 * (FR-030); its mode does not apply.
 */
import { computed, provide, ref, useTemplateRef } from 'vue'
import { DOCK_HOLD } from '~/composables/useDock'
import { resolveDockEntries } from '~/lib/wm/dock'

const wm = useWindowManagerStore()
const dock = useDock()

const entries = computed(() => resolveDockEntries(dock.items.value, wm.windows))
const placement = dock.effective
const vertical = computed(
  () => placement.value.edge === 'left' || placement.value.edge === 'right',
)
const reserved = computed(
  () => placement.value.style === 'bar' && placement.value.mode === 'reserved',
)
const autohide = computed(
  () => placement.value.style === 'bar' && placement.value.mode === 'autohide',
)

const JUSTIFY = {
  start: 'justify-start',
  center: 'justify-center',
  end: 'justify-end',
} as const
const ITEMS = {
  top: 'items-start',
  bottom: 'items-end',
  left: 'items-start',
  right: 'items-end',
} as const
const HIDDEN = {
  top: '-translate-y-[calc(100%+1rem)]',
  bottom: 'translate-y-[calc(100%+1rem)]',
  left: '-translate-x-[calc(100%+1rem)]',
  right: 'translate-x-[calc(100%+1rem)]',
} as const
const STRIP = {
  top: 'inset-x-0 top-0 h-1',
  bottom: 'inset-x-0 bottom-0 h-1',
  left: 'inset-y-0 left-0 w-1',
  right: 'inset-y-0 right-0 w-1',
} as const

const frameClass = computed(() => [
  'flex p-2',
  vertical.value ? 'flex-col' : 'flex-row',
  JUSTIFY[placement.value.align],
  reserved.value
    ? 'shrink-0'
    : `pointer-events-none absolute inset-0 z-10 ${ITEMS[placement.value.edge]}`,
])

// Auto-hide (FR-024, FR-025).
const bar = useTemplateRef<HTMLElement>('bar')
const { focused } = useFocusWithin(bar)
const pointerInside = ref(false)
const held = ref(0)
const revealed = ref(false)
let hideTimer: ReturnType<typeof setTimeout> | undefined

function hold(open: boolean) {
  held.value = Math.max(0, held.value + (open ? 1 : -1))
  if (!open) scheduleHide()
}
provide(DOCK_HOLD, hold)

const shown = computed(
  () => !autohide.value || revealed.value || focused.value || held.value > 0,
)

function reveal() {
  clearTimeout(hideTimer)
  revealed.value = true
}

function scheduleHide() {
  clearTimeout(hideTimer)
  hideTimer = setTimeout(() => {
    if (!pointerInside.value && held.value === 0) revealed.value = false
  }, 400)
}

function onPointerenter() {
  pointerInside.value = true
  reveal()
}

function onPointerleave() {
  pointerInside.value = false
  scheduleHide()
}
</script>

<template>
  <div
    :class="frameClass"
    data-testid="dock"
    :data-style="placement.style"
    :data-edge="placement.edge"
    :data-align="placement.align"
    :data-mode="placement.mode"
  >
    <div
      v-if="autohide"
      class="pointer-events-auto absolute"
      :class="STRIP[placement.edge]"
      aria-hidden="true"
      @pointerenter="reveal"
    />
    <div v-if="placement.style === 'wheel'" class="pointer-events-auto">
      <WmDockWheel
        :entries="entries"
        :edge="placement.edge"
        :align="placement.align"
      />
    </div>
    <ShadcnContextMenu v-else @update:open="hold">
      <ShadcnContextMenuTrigger as-child>
        <div
          ref="bar"
          class="pointer-events-auto max-h-full max-w-full transition-transform duration-200 motion-reduce:transition-none"
          :class="shown ? '' : HIDDEN[placement.edge]"
          @pointerenter="onPointerenter"
          @pointerleave="onPointerleave"
        >
          <WmDockBar
            :entries="entries"
            :orientation="vertical ? 'vertical' : 'horizontal'"
          />
        </div>
      </ShadcnContextMenuTrigger>
      <ShadcnContextMenuContent class="min-w-48" data-testid="dock-menu">
        <WmDockPlacementMenu />
      </ShadcnContextMenuContent>
    </ShadcnContextMenu>
  </div>
</template>
