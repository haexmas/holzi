<script setup lang="ts">
/**
 * Back/forward for the window's active tab (spec 020-tab-navigation,
 * T023/T034, FR-015/FR-016, contracts/wm-actions.md §6), left of the tab
 * bar. Each button is enabled only when the tab's own history has an entry in
 * that direction and triggers the catalog action with this tab as explicit
 * target. A long press (≥ 500 ms) or a right click opens the history list of
 * that direction instead. Both stay visible in compact mode, with the larger
 * touch size there.
 */
import { computed, ref } from 'vue'
import { useAction } from '~/composables/useAction'
import { canGoBack, canGoForward } from '~/lib/wm/navigation'

type Direction = 'back' | 'forward'

const LONG_PRESS_MS = 500

const props = defineProps<{
  tabId: string
  appId: string
  compact: boolean
}>()

const wm = useWindowManagerStore()
const { t } = useI18n()
const runBack = useAction('wm.tab.back')
const runForward = useAction('wm.tab.forward')
const runGo = useAction('wm.tab.go')

const history = computed(() => wm.historyOf(props.tabId))
const enabled = computed(() => ({
  back: history.value ? canGoBack(history.value) : false,
  forward: history.value ? canGoForward(history.value) : false,
}))
const menuOpen = ref<Record<Direction, boolean>>({
  back: false,
  forward: false,
})

const isMac =
  typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac')
const shortcut: Record<Direction, string> = isMac
  ? { back: '⌘[', forward: '⌘]' }
  : { back: 'Alt+←', forward: 'Alt+→' }
const label = computed<Record<Direction, string>>(() => ({
  back: t('wm.nav.back'),
  forward: t('wm.nav.forward'),
}))
const buttonClass = computed(() => [
  'rounded text-muted-foreground hover:bg-accent hover:text-foreground disabled:pointer-events-none disabled:opacity-40',
  props.compact ? 'p-2' : 'p-1',
])

let pressTimer: ReturnType<typeof setTimeout> | null = null
let suppressClick = false

function openMenu(direction: Direction) {
  if (enabled.value[direction]) menuOpen.value[direction] = true
}

function onPointerDown(event: PointerEvent, direction: Direction) {
  if (event.button !== 0) return
  suppressClick = false
  pressTimer = setTimeout(() => {
    pressTimer = null
    suppressClick = true
    openMenu(direction)
  }, LONG_PRESS_MS)
}

function cancelPress() {
  if (pressTimer !== null) clearTimeout(pressTimer)
  pressTimer = null
}

function onClick(direction: Direction) {
  if (suppressClick) {
    suppressClick = false
    return
  }
  const run = direction === 'back' ? runBack : runForward
  void run({ tabId: props.tabId })
}

function onSelect(steps: number) {
  void runGo({ tabId: props.tabId, steps })
}
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5" @pointerdown.stop>
    <div
      v-for="direction in ['back', 'forward'] as const"
      :key="direction"
      class="relative"
    >
      <button
        type="button"
        :class="buttonClass"
        :disabled="!enabled[direction]"
        :aria-label="label[direction]"
        :data-testid="`nav-${direction}`"
        :title="`${label[direction]} (${shortcut[direction]})`"
        aria-haspopup="menu"
        @pointerdown="onPointerDown($event, direction)"
        @pointerup="cancelPress"
        @pointerleave="cancelPress"
        @contextmenu.prevent="openMenu(direction)"
        @click="onClick(direction)"
      >
        <Icon
          :name="
            direction === 'back' ? 'lucide:arrow-left' : 'lucide:arrow-right'
          "
          class="h-3.5 w-3.5"
          :aria-hidden="true"
        />
      </button>
      <WmHistoryMenu
        v-model:open="menuOpen[direction]"
        :tab-id="tabId"
        :app-id="appId"
        :direction="direction"
        @select="onSelect"
      />
    </div>
  </div>
</template>
