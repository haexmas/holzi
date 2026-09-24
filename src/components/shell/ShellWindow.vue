<script setup lang="ts">
/**
 * One Shell window: frame, title bar (icon, title, minimize/maximize/close),
 * drag-to-move, eight-way resize, and its app content (spec
 * 015-workspace-shell, T023 + T029 + T031). The Firefox-style tab bar/Chevron
 * (FR-007's tab area, FR-031-038) is User Story 3 and extends this same
 * file (T034-T037).
 *
 * Hidden via `v-show`, never unmounted, when minimized — its content is the
 * same (research R8): lazy-mounted the first time it becomes visible
 * (`shell.markTabMounted`), then left mounted regardless of later
 * minimize/focus/tab/workspace changes.
 *
 * Provides the `useShellTab()` contract for the window's one tab (Phase 3
 * has no multi-tab windows yet — that is tabs.ts, T033).
 */
import { computed, watchEffect } from 'vue'
import { getAppDefinition } from '~/lib/shell/apps'
import { getAppComponent } from '~/components/shell/appComponents'
import { windowDisplayRect } from '~/lib/shell/geometry'
import { provideShellTab, requestCloseWindow } from '~/composables/useShellTab'
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

const tab = computed(() => props.window.tabs[0])
const app = computed(() =>
  tab.value ? getAppDefinition(tab.value.appId) : undefined,
)
const component = computed(() =>
  tab.value ? getAppComponent(tab.value.appId) : undefined,
)
const info = computed(() => shell.windowDisplayInfo(props.window))
const title = computed(() => {
  const displayInfo = info.value
  if (!displayInfo) return ''
  return (
    displayInfo.titleOverride ??
    (displayInfo.titleKey ? t(displayInfo.titleKey) : '')
  )
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
  () => app.value?.minSize ?? { width: 0, height: 0 },
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
  void requestCloseWindow(shell, t, props.window.id)
}

// Lazy-mount (research R8): marks the tab mounted the moment it exists, which for this phase's
// single-tab-per-window is immediately on open; re-runs if the active tab ever changes (tabs.ts,
// T033), covering the same rule for a tab that was restored from persistence but never activated.
watchEffect(() => {
  if (tab.value) shell.markTabMounted(tab.value.id)
})

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

provideShellTab({
  get tabId() {
    return tab.value?.id ?? ''
  },
  get windowId() {
    return props.window.id
  },
  requestAttention: () => {
    if (tab.value) shell.setTabAttention(tab.value.id, true)
  },
  clearAttention: () => {
    if (tab.value) shell.setTabAttention(tab.value.id, false)
  },
  setTitle: (value) => {
    if (tab.value) shell.setTabTitle(tab.value.id, value)
  },
  registerCloseGuard: (guard) => {
    const registeredFor = tab.value?.id
    if (registeredFor) shell.setTabCloseGuard(registeredFor, guard)
    return () => {
      if (registeredFor) shell.setTabCloseGuard(registeredFor, null)
    }
  },
  // One tab per window in this phase — closing the tab closes the window
  // (tabs.ts, T033, replaces this with the neighbor-activation rule).
  closeSelf: () => shell.closeWindow(props.window.id),
})
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
      class="flex shrink-0 items-center gap-2 border-b border-border bg-muted/40 px-3 py-2"
      :style="interactive ? { touchAction: 'none' } : undefined"
      @pointerdown="onTitleBarPointerDown"
      @dblclick="onTitleBarDoubleClick"
    >
      <Icon
        v-if="app"
        :name="app.icon"
        class="h-3.5 w-3.5 shrink-0"
        :aria-hidden="true"
      />
      <span class="min-w-0 flex-1 truncate text-sm font-medium">{{
        title
      }}</span>
      <span
        v-if="info?.hasAttention"
        class="h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500"
        :aria-label="t('shell.attention')"
      />
      <ShellWindowControls
        :maximized="window.maximized"
        @minimize="shell.minimizeWindow(window.id)"
        @toggle-maximize="shell.toggleMaximizeWindow(window.id)"
        @close="requestClose"
      />
    </div>
    <div class="min-h-0 flex-1 overflow-hidden">
      <div
        v-if="tab"
        :key="tab.id"
        role="tabpanel"
        class="h-full min-h-0"
        :aria-label="title"
      >
        <component
          :is="component"
          v-if="component && shell.runtimeFor(tab.id).mounted"
        />
      </div>
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
