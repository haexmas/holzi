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
 * The overlay open states live in the store (`shell.overlays`, spec 020) so
 * system back can close them and open the window overview.
 */
import { watch } from 'vue'

const shell = useShellStore()
const { t } = useI18n()
const { width, height } = useWindowSize()

watch([width, height], ([newWidth, newHeight]) => {
  shell.updateArea({ width: newWidth, height: newHeight })
})

const openWorkspaces = useAction('shell.workspaces.overview')
const openWindows = useAction('shell.windows.overview')
const openLauncher = useAction('shell.launcher.open')
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
        @click="openWorkspaces()"
      >
        <Icon name="lucide:monitor" class="h-5 w-5" :aria-hidden="true" />
      </button>
      <button
        type="button"
        class="flex h-12 w-12 items-center justify-center rounded-full bg-background text-foreground shadow-lg ring-1 ring-border hover:bg-accent"
        :aria-label="t('shell.windowOverview.open')"
        @click="openWindows()"
      >
        <Icon name="lucide:copy" class="h-5 w-5" :aria-hidden="true" />
      </button>
      <button
        type="button"
        class="flex h-12 w-12 items-center justify-center rounded-full bg-foreground text-background shadow-lg hover:opacity-90"
        data-testid="open-launcher"
        :aria-label="t('shell.launcher.open')"
        @click="openLauncher()"
      >
        <Icon name="lucide:layout-grid" class="h-5 w-5" :aria-hidden="true" />
      </button>
    </div>

    <ShellLauncher v-model:open="shell.overlays.launcher" />
    <ShellWindowOverview v-model:open="shell.overlays.windows" />
    <ShellWorkspaceOverview v-model:open="shell.overlays.workspaces" />
    <ShellCloseConfirm />
  </div>
</template>
