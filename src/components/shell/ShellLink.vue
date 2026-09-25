<script setup lang="ts">
/**
 * An in-tab link (spec 020-tab-navigation, T019,
 * contracts/tab-navigation-contract.md §3). Triggers the
 * `shell.tab.navigate` action for its own tab (FR-024) and never gives the
 * webview a navigation target (`href="#"`). `aria-current="page"` marks the
 * current location — with `prefix`, also any location below it, which is how
 * a sidebar highlights the category of the current view (US1 AS7).
 */
import { computed } from 'vue'
import { useAction } from '~/composables/useAction'
import { useShellTab } from '~/composables/useShellTab'
import { useTabRouter } from '~/composables/useTabRouter'
import { isLocationActive } from '~/lib/shell/navigation'

const props = withDefaults(
  defineProps<{ to: string; replace?: boolean; prefix?: boolean }>(),
  { replace: false, prefix: false },
)

const tab = useShellTab()
const router = useTabRouter()
const navigate = useAction('shell.tab.navigate')

const active = computed(() =>
  isLocationActive(router.route.path, props.to, props.prefix),
)

function onClick() {
  void navigate({ tabId: tab.tabId, to: props.to, replace: props.replace })
}
</script>

<template>
  <a
    href="#"
    :aria-current="active ? 'page' : undefined"
    @click.prevent="onClick"
  >
    <slot />
  </a>
</template>
