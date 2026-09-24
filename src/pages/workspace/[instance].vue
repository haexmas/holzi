<script setup lang="ts">
/**
 * Shell host page (spec 015-workspace-shell, T023). Replaces the spec-002
 * workspace stub: onboarding enforcement (FR-001) stays here since this is
 * now the only real page apps are reached through (chat/settings/
 * federation moved into Shell apps, T021-T022). The Shell status bar
 * (model preload status, FR-005) is added in T025.
 *
 * Consumes `?open=<appId>` once (contracts/shell-app-contract.md §3, T024's
 * legacy-route redirects land here with it set) and removes it via
 * `router.replace` so it does not re-fire and open a second tab on a
 * later navigation that happens to keep it in the URL.
 */
definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const router = useRouter()
const instancesStore = useInstancesStore()
const shell = useShellStore()

const instanceName = computed(() => {
  const raw = route.params.instance
  return typeof raw === 'string'
    ? raw
    : Array.isArray(raw)
      ? (raw[0] ?? '')
      : ''
})

// Normally already set by the caller (pages/index.vue) before navigating
// here; set again so a direct/refreshed load of this route still resolves
// the same instance for components that only read the store (ChatApp.vue
// and friends have no route of their own).
onMounted(() => {
  instancesStore.setActiveInstance(instanceName.value)

  const open = route.query.open
  if (typeof open === 'string' && open.length > 0) {
    shell.openApp(open)
    const { open: _discarded, ...rest } = route.query
    void router.replace({ query: rest })
  }
})
</script>

<template>
  <div class="flex h-screen min-h-0 flex-col">
    <ShellDesktop class="min-h-0 flex-1" />
  </div>
</template>
