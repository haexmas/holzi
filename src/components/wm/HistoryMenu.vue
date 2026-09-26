<script setup lang="ts">
/**
 * The history list of one tab in one direction (spec 020-tab-navigation,
 * T034, FR-016): at most 15 entries, nearest first, each with the title it
 * showed when it was left (or the title of its location). Controlled by
 * `open` — `wm/NavButtons.vue` opens it on long press or right click; the
 * invisible trigger only anchors the menu under the button. A selection
 * emits the signed step count for `wm.tab.go`.
 */
import { computed } from 'vue'
import { titleForLocation } from '~/components/wm/appRoutes'
import { backList, forwardList } from '~/lib/wm/navigation'

const props = defineProps<{
  tabId: string
  appId: string
  direction: 'back' | 'forward'
}>()

const open = defineModel<boolean>('open', { default: false })

const emit = defineEmits<{
  select: [steps: number]
}>()

const wm = useWindowManagerStore()
const { t } = useI18n()

/** The menu's accessible name. reka-ui names the menu by its trigger
 * (`aria-labelledby`), and an `aria-label` on `ShadcnDropdownMenuContent`
 * would land on its portal root and be dropped; so the invisible trigger
 * carries the name as screen-reader text. */
const label = computed(() =>
  props.direction === 'back' ? t('wm.nav.backList') : t('wm.nav.forwardList'),
)

const items = computed(() => {
  const history = wm.historyOf(props.tabId)
  if (!history) return []
  const list =
    props.direction === 'back' ? backList(history) : forwardList(history)
  return list.map((item) => {
    const { key, params } = titleForLocation(
      props.appId,
      item.entry.location.path,
    )
    return {
      steps: item.steps,
      title:
        item.entry.title ?? (key ? t(key, params) : item.entry.location.path),
    }
  })
})
</script>

<template>
  <ShadcnDropdownMenu v-model:open="open">
    <ShadcnDropdownMenuTrigger as-child>
      <span class="pointer-events-none absolute inset-0" aria-hidden="true">
        <span class="sr-only">{{ label }}</span>
      </span>
    </ShadcnDropdownMenuTrigger>
    <ShadcnDropdownMenuContent align="start">
      <ShadcnDropdownMenuItem
        v-for="item in items"
        :key="item.steps"
        @select="emit('select', item.steps)"
      >
        <span class="min-w-0 max-w-64 truncate">{{ item.title }}</span>
      </ShadcnDropdownMenuItem>
    </ShadcnDropdownMenuContent>
  </ShadcnDropdownMenu>
</template>
