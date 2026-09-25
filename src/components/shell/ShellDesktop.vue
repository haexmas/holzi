<script setup lang="ts">
/**
 * The Shell's desktop area: every window of the active workspace, plus the
 * Launcher, window-overview and workspace-overview triggers (spec
 * 015-workspace-shell, T023, T030, T040, T049, FR-002, FR-011, FR-022).
 *
 * `useWindowSize` keeps `shell.area`/`shell.compact` live (T049) — the
 * store's initial value already matches it at setup time (`window.
 * innerWidth`/`innerHeight`), so only actual *changes* need forwarding;
 * `updateArea` itself re-clamps window geometry into the new area
 * (`layoutState.ts`), `ShellWindow.vue`'s `windowDisplayRect` already
 * handles the compact/normal display switch without this component's help.
 */
import { ref, watch } from 'vue'

const shell = useShellStore()
const { t } = useI18n()
const { width, height } = useWindowSize()

watch([width, height], ([newWidth, newHeight]) => {
  shell.updateArea({ width: newWidth, height: newHeight })
})

const launcherOpen = ref(false)
const windowOverviewOpen = ref(false)
const workspaceOverviewOpen = ref(false)
</script>

<template>
  <div class="relative h-full min-h-0 w-full overflow-hidden bg-muted/10">
    <div class="absolute inset-0 isolate">
      <ShellWindow
        v-for="win in shell.windowsInActiveWorkspace"
        :key="win.id"
        :window="win"
        :active="win.id === shell.activeWindowId"
      />
    </div>

    <div class="absolute bottom-4 right-4 z-10 flex flex-col gap-2">
      <button
        type="button"
        class="flex h-12 w-12 items-center justify-center rounded-full bg-background text-foreground shadow-lg ring-1 ring-border hover:bg-accent"
        :aria-label="t('shell.workspaces.title')"
        @click="workspaceOverviewOpen = true"
      >
        <Icon name="lucide:layout-list" class="h-5 w-5" :aria-hidden="true" />
      </button>
      <button
        type="button"
        class="flex h-12 w-12 items-center justify-center rounded-full bg-background text-foreground shadow-lg ring-1 ring-border hover:bg-accent"
        :aria-label="t('shell.windowOverview.open')"
        @click="windowOverviewOpen = true"
      >
        <Icon
          name="lucide:layout-panel-top"
          class="h-5 w-5"
          :aria-hidden="true"
        />
      </button>
      <button
        type="button"
        class="flex h-12 w-12 items-center justify-center rounded-full bg-foreground text-background shadow-lg hover:opacity-90"
        :aria-label="t('shell.launcher.open')"
        @click="launcherOpen = true"
      >
        <Icon name="lucide:layout-grid" class="h-5 w-5" :aria-hidden="true" />
      </button>
    </div>

    <ShellLauncher v-model:open="launcherOpen" />
    <ShellWindowOverview v-model:open="windowOverviewOpen" />
    <ShellWorkspaceOverview v-model:open="workspaceOverviewOpen" />
    <ShellCloseConfirm />
  </div>
</template>
