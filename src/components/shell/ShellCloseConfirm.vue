<script setup lang="ts">
/**
 * The one real confirmation dialog for FR-014 close guards, replacing the
 * `window.confirm` placeholders in `useShellTab.ts`'s `requestCloseWindow`
 * and `ShellWindow.vue`'s tab-close handler. Aggregates every guard result
 * for the close action into one dialog naming each reason (spec
 * 015-workspace-shell, T038, plan research R14: `ShadcnAlertDialog`).
 * Mounted once, in `ShellDesktop.vue` — `useShellCloseConfirm.ts` holds the
 * (module-level, single-pending) state any close action feeds into.
 *
 * Also used for workspace deletion (T040, FR-021): that reason has no
 * guard/`confirmAsync` of its own, just an interpolated window count.
 */
import {
  resolvePending,
  usePendingCloseConfirmation,
  type ConfirmationReason,
} from '~/composables/useShellCloseConfirm'

const pending = usePendingCloseConfirmation()
const { t } = useI18n()

/** vue-i18n only pluralizes from a positional number argument, not from a `count` inside a named-
 * params object — this passes both when `params.count` is a number (T040's window-count reason),
 * plain named interpolation otherwise (T038's guard reasons). */
function describe(reason: ConfirmationReason): string {
  const count = reason.params?.count
  if (typeof count === 'number')
    return t(reason.reasonKey, reason.params ?? {}, count)
  return t(reason.reasonKey, reason.params ?? {})
}
</script>

<template>
  <ShadcnAlertDialog
    :open="pending !== null"
    @update:open="(open) => !open && resolvePending(false)"
  >
    <ShadcnAlertDialogContent v-if="pending">
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>{{
          t('shell.close.confirmTitle')
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription as-child>
          <ul class="list-disc space-y-1 pl-4">
            <li v-for="(reason, index) in pending.reasons" :key="index">
              {{ describe(reason) }}
            </li>
          </ul>
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel @click="resolvePending(false)">
          {{ t('shell.close.cancel') }}
        </ShadcnAlertDialogCancel>
        <ShadcnAlertDialogAction @click="resolvePending(true)">
          {{ t('shell.close.confirm') }}
        </ShadcnAlertDialogAction>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
