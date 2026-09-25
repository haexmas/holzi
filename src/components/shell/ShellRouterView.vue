<script setup lang="ts">
/**
 * Renders the route record matched at its own nesting depth for the current
 * tab location (spec 020-tab-navigation, T019, research R4,
 * contracts/tab-navigation-contract.md §3). The root instance lives in
 * `ShellTabPanel.vue`; an app places a nested one where its child views
 * appear. The component at a depth only changes when the matched record does,
 * so a root component stays mounted while its children change.
 *
 * At the root, an unknown location falls back to the app's start `/` with an
 * unobtrusive hint (FR-014).
 */
import { computed, inject, provide, watch } from 'vue'
import { toast } from 'vue-sonner'
import { getAppRoutes } from '~/components/shell/appRoutes'
import { useShellTab } from '~/composables/useShellTab'
import { ROUTER_DEPTH_KEY, useTabRouter } from '~/composables/useTabRouter'
import { matchRoute } from '~/lib/shell/routeMatch'

const depth = inject(ROUTER_DEPTH_KEY, 0)
provide(ROUTER_DEPTH_KEY, depth + 1)

const tab = useShellTab()
const router = useTabRouter()
const { t } = useI18n()

const match = computed(() =>
  matchRoute(getAppRoutes(tab.appId) ?? [], router.route.path),
)
const component = computed(() => match.value?.chain[depth]?.component)

if (depth === 0) {
  watch(
    () => [match.value, router.route.path] as const,
    ([matched, path]) => {
      if (matched || path === '/') return
      router.replace('/')
      toast(t('shell.nav.unknownLocation'))
    },
    { immediate: true },
  )
}
</script>

<template>
  <component :is="component" v-if="component" />
</template>
