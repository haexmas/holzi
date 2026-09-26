<script setup lang="ts">
/**
 * window manager host page (spec 015-workspace-shell, T023). Replaces the spec-002
 * workspace stub: onboarding enforcement (FR-001) stays here since this is
 * now the only real page apps are reached through (chat/settings/
 * federation moved into window manager apps, T021-T022). The model status shows in
 * the chat only; the workspace-wide status bar (015 FR-005) was dropped by
 * operator decision.
 *
 * Awaits `wm.restoreSessionAsync()` (spec 022) before anything else: with
 * the setting "Sitzung wiederherstellen" on, the saved session replaces the
 * store's initial empty workspace, and a window opened before that would be
 * discarded. A failure is logged and the page carries on with the empty
 * start, so a deep link below still opens its app (spec 022 FR-012, FR-014).
 *
 * Consumes `?open=<appId>` (and spec 020's optional `&at=<path>`) once (contracts/shell-app-contract.md §3, T024's
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
const wm = useWindowManagerStore()
// Spec 020: create the models store here, inside a component setup (its setup calls `useI18n()`),
// so the global chat actions (`stores/chatActionHandlers.ts`) find it when an action runs.
useModelsStore()

// Spec 020: global shortcuts for window manager actions (back/forward).
useWmKeyboard()

// Spec 020 (research R7, FR-020, FR-035): the webview history is never navigation state. Pages
// reach this one with `replace`, so the top document's history stays flat, and every router
// navigation away from it — a webview back, or an embedded document's `history.back()` — is
// cancelled and deliberately not read as a tab's back (locking or closing ends the process,
// spec 013, so there is no legitimate route away).
onBeforeRouteLeave(() => false)

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
onMounted(async () => {
  instancesStore.setActiveInstance(instanceName.value)
  try {
    await wm.restoreSessionAsync()
  } catch (error) {
    console.error('[wm] restoring the session failed; starting empty', error)
  }

  const open = route.query.open
  if (typeof open === 'string' && open.length > 0) {
    // Spec 020 FR-012/FR-013: `&at=<path>` opens the app at a location.
    const at = route.query.at
    void wm.runAction('wm.app.open', {
      appId: open,
      ...(typeof at === 'string' && at.length > 0 ? { at } : {}),
    })
    const { open: _open, at: _at, ...rest } = route.query
    void router.replace({ query: rest })
  }
})
</script>

<template>
  <div class="flex h-screen min-h-0 flex-col">
    <WmDesktop class="min-h-0 flex-1" />
  </div>
</template>
