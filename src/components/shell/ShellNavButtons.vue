<script setup lang="ts">
/**
 * Back/forward for the window's active tab (spec 020-tab-navigation, T023,
 * FR-015, contracts/shell-actions.md §6), left of the tab bar. Each button is
 * enabled only when the tab's own history has an entry in that direction and
 * triggers the catalog action with this tab as explicit target. Both stay
 * visible in compact mode, with the larger touch size there.
 */
import { computed } from 'vue'
import { useAction } from '~/composables/useAction'
import { canGoBack, canGoForward } from '~/lib/shell/navigation'

const props = defineProps<{
  tabId: string
  compact: boolean
}>()

const shell = useShellStore()
const { t } = useI18n()
const back = useAction('shell.tab.back')
const forward = useAction('shell.tab.forward')

const history = computed(() => shell.historyOf(props.tabId))
const backEnabled = computed(() =>
  history.value ? canGoBack(history.value) : false,
)
const forwardEnabled = computed(() =>
  history.value ? canGoForward(history.value) : false,
)
const isMac =
  typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac')
const buttonClass = computed(() => [
  'rounded text-muted-foreground hover:bg-accent hover:text-foreground disabled:pointer-events-none disabled:opacity-40',
  props.compact ? 'p-2' : 'p-1',
])
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5" @pointerdown.stop>
    <button
      type="button"
      :class="buttonClass"
      :disabled="!backEnabled"
      :aria-label="t('shell.nav.back')"
      :title="`${t('shell.nav.back')} (${isMac ? '⌘[' : 'Alt+←'})`"
      @click="back({ tabId })"
    >
      <Icon name="lucide:arrow-left" class="h-3.5 w-3.5" :aria-hidden="true" />
    </button>
    <button
      type="button"
      :class="buttonClass"
      :disabled="!forwardEnabled"
      :aria-label="t('shell.nav.forward')"
      :title="`${t('shell.nav.forward')} (${isMac ? '⌘]' : 'Alt+→'})`"
      @click="forward({ tabId })"
    >
      <Icon name="lucide:arrow-right" class="h-3.5 w-3.5" :aria-hidden="true" />
    </button>
  </div>
</template>
