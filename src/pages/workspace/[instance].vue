<script setup lang="ts">
/**
 * Shell host page (spec 015-workspace-shell, T023). Replaces the spec-002
 * workspace stub: onboarding enforcement (FR-001) stays here since this is
 * now the only real page apps are reached through (chat/settings/
 * federation moved into Shell apps, T021-T022). The Shell status bar
 * (model preload status, FR-005) and the `?open=` query param are added in
 * T025.
 */
definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const instancesStore = useInstancesStore()

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
})
</script>

<template>
  <div class="flex h-screen min-h-0 flex-col">
    <ShellDesktop class="min-h-0 flex-1" />
  </div>
</template>
