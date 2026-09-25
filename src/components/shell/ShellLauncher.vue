<script setup lang="ts">
/**
 * Lists every registered app with its localized name and icon; selecting
 * one opens it as a window (FR-002). An attention dot marks an app with a
 * tab waiting on a reply anywhere (any window, any workspace) — the
 * Launcher is one of the surfaces FR-015 requires that to reach even when
 * the affected window is elsewhere. Compact mode (T050) drops to two
 * columns so each tile stays a comfortable touch target on a narrow
 * screen, instead of three cramped ones. Spec 015-workspace-shell, T023,
 * T041, T050.
 */
import { SHELL_APPS } from '~/lib/shell/apps'

const open = defineModel<boolean>('open', { default: false })

const shell = useShellStore()
const { t } = useI18n()

function launch(appId: string) {
  shell.openApp(appId)
  open.value = false
}
</script>

<template>
  <UiDrawerModal
    :open="open"
    :title="t('shell.launcher.title')"
    @update:open="open = $event"
  >
    <template #content>
      <div
        class="grid gap-3 overflow-x-hidden p-4"
        :class="shell.compact ? 'grid-cols-2' : 'grid-cols-3'"
      >
        <button
          v-for="app in SHELL_APPS"
          :key="app.id"
          type="button"
          class="relative flex flex-col items-center gap-2 rounded-lg p-3 text-sm text-foreground hover:bg-accent"
          :data-testid="app.id === 'system.chat' ? 'open-chat' : undefined"
          @click="launch(app.id)"
        >
          <span
            v-if="shell.appHasAttention(app.id)"
            class="absolute right-2 top-2 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500"
            :aria-label="t('shell.attention')"
          />
          <Icon :name="app.icon" class="h-6 w-6" :aria-hidden="true" />
          <span class="truncate">{{ t(app.titleKey) }}</span>
        </button>
      </div>
    </template>
  </UiDrawerModal>
</template>
