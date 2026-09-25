<script setup lang="ts">
/**
 * The history list of one tab in one direction (spec 020-tab-navigation,
 * T034, FR-016): at most 15 entries, nearest first, each with the title it
 * showed when it was left (or the title of its location). Controlled by
 * `open` — `ShellNavButtons.vue` opens it on long press or right click; the
 * invisible trigger only anchors the menu under the button. A selection
 * emits the signed step count for `shell.tab.go`.
 */
import { computed } from 'vue'
import { titleForLocation } from '~/components/shell/appRoutes'
import { backList, forwardList } from '~/lib/shell/navigation'

const props = defineProps<{
  tabId: string
  appId: string
  direction: 'back' | 'forward'
}>()

const open = defineModel<boolean>('open', { default: false })

const emit = defineEmits<{
  select: [steps: number]
}>()

const shell = useShellStore()
const { t } = useI18n()

const items = computed(() => {
  const history = shell.historyOf(props.tabId)
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
      <span class="pointer-events-none absolute inset-0" aria-hidden="true" />
    </ShadcnDropdownMenuTrigger>
    <ShadcnDropdownMenuContent
      align="start"
      :aria-label="
        direction === 'back'
          ? t('shell.nav.backList')
          : t('shell.nav.forwardList')
      "
    >
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
