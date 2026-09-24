<script setup lang="ts">
/**
 * The Shell's desktop area: every window of the active workspace, plus the
 * Launcher trigger (spec 015-workspace-shell, T023). Workspace switching,
 * the overview/status bar and compact-mode layout are later user stories
 * (Phase 6, Phase 8, T025) and extend this component then.
 */
import { ref } from 'vue'

const shell = useShellStore()
const { t } = useI18n()

const launcherOpen = ref(false)
</script>

<template>
  <div class="relative h-full min-h-0 w-full overflow-hidden bg-muted/10">
    <ShellWindow
      v-for="win in shell.windowsInActiveWorkspace"
      :key="win.id"
      :window="win"
      :active="win.id === shell.activeWindowId"
    />

    <button
      type="button"
      class="absolute bottom-4 right-4 flex h-12 w-12 items-center justify-center rounded-full bg-foreground text-background shadow-lg hover:opacity-90"
      :aria-label="t('shell.launcher.open')"
      @click="launcherOpen = true"
    >
      <Icon name="lucide:layout-grid" class="h-5 w-5" :aria-hidden="true" />
    </button>

    <ShellLauncher v-model:open="launcherOpen" />
  </div>
</template>
