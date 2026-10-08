<script setup lang="ts">
/**
 * Edge, alignment, style and mode of the dock as context-menu submenus (spec 045, FR-018), for the
 * free area of the bar and the wheel's button. Goes inside a `ShadcnContextMenuContent`.
 */
import type { DockPlacement } from '~/lib/wm/dock'

const dock = useDock()
const { t } = useI18n()

const GROUPS = [
  { field: 'edge', values: ['top', 'bottom', 'left', 'right'] },
  { field: 'align', values: ['start', 'center', 'end'] },
  { field: 'style', values: ['bar', 'wheel'] },
  { field: 'mode', values: ['reserved', 'floating', 'autohide'] },
] as const

function set(field: keyof DockPlacement, value: unknown) {
  void dock.setPlacementAsync({ [field]: value })
}
</script>

<template>
  <template v-for="group in GROUPS" :key="group.field">
    <ShadcnContextMenuSub>
      <ShadcnContextMenuSubTrigger
        :data-testid="`dock-placement-${group.field}`"
      >
        {{ t(`settings.dock.${group.field}`) }}
      </ShadcnContextMenuSubTrigger>
      <ShadcnContextMenuSubContent>
        <ShadcnContextMenuRadioGroup
          :model-value="dock.placement.value[group.field]"
          @update:model-value="(value) => set(group.field, value)"
        >
          <ShadcnContextMenuRadioItem
            v-for="value in group.values"
            :key="value"
            :value="value"
            :data-testid="`dock-placement-${group.field}-${value}`"
          >
            {{ t(`settings.dock.${group.field}Options.${value}`) }}
          </ShadcnContextMenuRadioItem>
        </ShadcnContextMenuRadioGroup>
      </ShadcnContextMenuSubContent>
    </ShadcnContextMenuSub>
  </template>
</template>
