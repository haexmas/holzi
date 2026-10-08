<script setup lang="ts">
/**
 * One entry of the dock (spec 045): a control (Launcher, window overview, workspace overview,
 * FR-014) or an app (FR-004, FR-009, FR-010), with its context menu (FR-015). Right click and a long
 * press on touch both open the menu (reka, FR-043).
 */
import { computed } from 'vue'
import type { DockControlId, DockEntry } from '~/lib/wm/dock'

const props = defineProps<{
  entry: DockEntry
  /** Roving tab stop of the dock's toolbar (FR-041): only one entry is reached with Tab. */
  tabbable: boolean
}>()

const wm = useWindowManagerStore()
const dock = useDock()
const { t } = useI18n()

const CONTROL_ICONS: Record<DockControlId, string> = {
  launcher: 'lucide:layout-grid',
  workspaces: 'lucide:monitor',
  windows: 'lucide:copy',
}
const CONTROL_ACTIONS = {
  launcher: useAction('wm.launcher.open'),
  workspaces: useAction('wm.workspaces.overview'),
  windows: useAction('wm.windows.overview'),
} satisfies Record<DockControlId, unknown>
const openApp = useAction('wm.app.open')

const app = computed(() => {
  const entry = props.entry
  if (entry.kind !== 'app') return undefined
  return wm.apps().find((candidate) => candidate.id === entry.appId)
})

const label = computed(() => {
  if (props.entry.kind === 'control')
    return t(`wm.dock.controls.${props.entry.id}`)
  return app.value ? (app.value.title ?? t(app.value.titleKey)) : ''
})

const testId = computed(() => {
  if (props.entry.kind === 'app') return `dock-item-${props.entry.appId}`
  return props.entry.id === 'launcher'
    ? 'open-launcher'
    : `dock-control-${props.entry.id}`
})

function activate() {
  const entry = props.entry
  if (entry.kind === 'control') {
    void CONTROL_ACTIONS[entry.id]({})
    return
  }
  void openApp({ appId: entry.appId })
}

function unpin() {
  if (props.entry.kind === 'app') void dock.unpinAsync(props.entry.appId)
}
</script>

<template>
  <ShadcnContextMenu>
    <ShadcnContextMenuTrigger as-child :disabled="entry.kind === 'control'">
      <button
        type="button"
        class="relative flex h-12 w-12 shrink-0 items-center justify-center rounded-full"
        :class="
          entry.kind === 'control' && entry.id === 'launcher'
            ? 'bg-foreground text-background hover:opacity-90'
            : 'text-foreground hover:bg-accent'
        "
        :data-testid="testId"
        data-dock-item
        :tabindex="tabbable ? 0 : -1"
        :aria-label="label"
        :title="label"
        @click="activate"
      >
        <img
          v-if="app?.iconUrl"
          :src="app.iconUrl"
          alt=""
          class="h-6 w-6 object-contain"
        />
        <Icon
          v-else
          :name="
            entry.kind === 'control'
              ? CONTROL_ICONS[entry.id]
              : (app?.icon ?? '')
          "
          class="h-5 w-5"
          :aria-hidden="true"
        />
      </button>
    </ShadcnContextMenuTrigger>
    <ShadcnContextMenuContent class="min-w-48" data-testid="dock-item-menu">
      <ShadcnContextMenuItem data-testid="dock-menu-unpin" @select="unpin">
        {{ t('wm.dock.unpin') }}
      </ShadcnContextMenuItem>
    </ShadcnContextMenuContent>
  </ShadcnContextMenu>
</template>
