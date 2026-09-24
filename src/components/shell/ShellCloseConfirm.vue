<script setup lang="ts">
/**
 * The one real confirmation dialog for FR-014 close guards, replacing the
 * `window.confirm` placeholders in `useShellTab.ts`'s `requestCloseWindow`
 * and `ShellWindow.vue`'s tab-close handler. Aggregates every guard result
 * for the close action into one dialog naming each reason (spec
 * 015-workspace-shell, T038, plan research R14: `ShadcnAlertDialog`).
 * Mounted once, in `ShellDesktop.vue` — `useShellCloseConfirm.ts` holds the
 * (module-level, single-pending) state any close action feeds into.
 */
import {
  resolvePending,
  usePendingCloseConfirmation,
} from '~/composables/useShellCloseConfirm'

const pending = usePendingCloseConfirmation()
const { t } = useI18n()
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
            <li v-for="(result, index) in pending.results" :key="index">
              {{ t(result.reasonKey) }}
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
