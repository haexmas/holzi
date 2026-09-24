<script setup lang="ts">
/**
 * One Shell window: frame, title bar (`ShellTabBar` + minimize/maximize/
 * close), drag-to-move, eight-way resize, and its tabs' content (spec
 * 015-workspace-shell, T023 + T029 + T031 + T034). The Chevron tab-list
 * menu (FR-034) and compact-mode title/Chevron swap (FR-036) are T036/T037
 * and extend this same file.
 *
 * Each tab renders through its own `ShellTabPanel` (T034) rather than this
 * component providing `useShellTab()` itself, so Vue's provide/inject stays
 * correctly scoped per tab even with several mounted at once (research R8).
 */
import { computed } from 'vue'
import { getAppDefinition } from '~/lib/shell/apps'
import { windowDisplayRect } from '~/lib/shell/geometry'
import {
  requestCloseTab as requestCloseTabAction,
  requestCloseWindow,
} from '~/composables/useShellTab'
import {
  useWindowPointerGesture,
  type ResizeDirection,
} from '~/composables/useWindowPointerGesture'
import type { ShellWindow } from '~/lib/shell/types'

const props = defineProps<{
  window: ShellWindow
  active: boolean
}>()

const shell = useShellStore()
const { t } = useI18n()

const info = computed(() => shell.windowDisplayInfo(props.window))
const title = computed(() => {
  const displayInfo = info.value
  if (!displayInfo) return ''
  return (
    displayInfo.titleOverride ??
    (displayInfo.titleKey ? t(displayInfo.titleKey) : '')
  )
})

// The largest minimum size across all of the window's tabs' apps, so it never shrinks below what
// any of them needs — not just the currently active one.
const minSize = computed(() => {
  let width = 0
  let height = 0
  for (const tab of props.window.tabs) {
    const app = getAppDefinition(tab.appId)
    if (app) {
      width = Math.max(width, app.minSize.width)
      height = Math.max(height, app.minSize.height)
    }
  }
  return { width, height }
})

const displayRect = computed(() =>
  windowDisplayRect(props.window, shell.compact, shell.area),
)
// Neither compact nor a maximized window offers moving/resizing (FR-028 for compact; a maximized
// window has nowhere to move or grow to until it is restored).
const interactive = computed(() => !shell.compact && !props.window.maximized)

const { startMove, startResize } = useWindowPointerGesture(
  () => ({
    x: props.window.x,
    y: props.window.y,
    width: props.window.width,
    height: props.window.height,
  }),
  () => minSize.value,
  () => shell.area,
  (geometry) => shell.updateWindowGeometry(props.window.id, geometry),
)

function onTitleBarPointerDown(event: PointerEvent) {
  if (interactive.value) startMove(event)
}

function onResizeHandlePointerDown(
  event: PointerEvent,
  direction: ResizeDirection,
) {
  if (interactive.value) startResize(event, direction)
}

function onTitleBarDoubleClick() {
  if (!shell.compact) shell.toggleMaximizeWindow(props.window.id)
}

function requestClose() {
  void requestCloseWindow(shell, props.window.id)
}

function selectTab(tabId: string) {
  shell.switchTab(props.window.id, tabId)
}

function requestCloseTab(tabId: string) {
  void requestCloseTabAction(shell, props.window.id, tabId)
}

const RESIZE_HANDLES: { direction: ResizeDirection; class: string }[] = [
  { direction: 'n', class: 'inset-x-2 top-0 h-1 cursor-ns-resize' },
  { direction: 's', class: 'inset-x-2 bottom-0 h-1 cursor-ns-resize' },
  { direction: 'e', class: 'inset-y-2 right-0 w-1 cursor-ew-resize' },
  { direction: 'w', class: 'inset-y-2 left-0 w-1 cursor-ew-resize' },
  { direction: 'ne', class: 'right-0 top-0 h-2 w-2 cursor-nesw-resize' },
  { direction: 'nw', class: 'left-0 top-0 h-2 w-2 cursor-nwse-resize' },
  { direction: 'se', class: 'right-0 bottom-0 h-2 w-2 cursor-nwse-resize' },
  { direction: 'sw', class: 'left-0 bottom-0 h-2 w-2 cursor-nesw-resize' },
]
</script>

<template>
  <div
    v-show="!window.minimized"
    class="absolute flex flex-col overflow-hidden rounded-lg border bg-background shadow-lg"
    :class="active ? 'border-foreground/40' : 'border-border'"
    :style="{
      left: `${displayRect.x}px`,
      top: `${displayRect.y}px`,
      width: `${displayRect.width}px`,
      height: `${displayRect.height}px`,
      zIndex: window.stack,
    }"
    role="group"
    :aria-label="title"
    @pointerdown="shell.focusWindow(window.id)"
  >
    <div
      class="flex shrink-0 items-center gap-1 border-b border-border bg-muted/40 px-1.5 py-1"
      :style="interactive ? { touchAction: 'none' } : undefined"
      @pointerdown="onTitleBarPointerDown"
      @dblclick="onTitleBarDoubleClick"
    >
      <ShellTabBar
        :window-id="window.id"
        :tabs="window.tabs"
        :active-tab-id="window.activeTabId"
        :compact="shell.compact"
        @select-tab="selectTab"
        @close-tab="requestCloseTab"
      />
      <div class="min-w-4 flex-1" />
      <ShellTabListMenu
        :tabs="window.tabs"
        :active-tab-id="window.activeTabId"
        @select-tab="selectTab"
      />
      <ShellWindowControls
        :maximized="window.maximized"
        :compact="shell.compact"
        @minimize="shell.minimizeWindow(window.id)"
        @toggle-maximize="shell.toggleMaximizeWindow(window.id)"
        @close="requestClose"
      />
    </div>
    <div class="min-h-0 flex-1 overflow-hidden">
      <ShellTabPanel
        v-for="tab in window.tabs"
        :key="tab.id"
        :tab="tab"
        :window-id="window.id"
        :active="tab.id === window.activeTabId"
      />
    </div>
    <div
      v-for="handle in RESIZE_HANDLES"
      v-show="interactive"
      :key="handle.direction"
      class="absolute"
      :class="handle.class"
      :style="{ touchAction: 'none' }"
      @pointerdown="onResizeHandlePointerDown($event, handle.direction)"
    />
  </div>
</template>
