<script setup lang="ts">
/**
 * The dock of the desktop (spec 045, FR-001): the stored entries plus the running apps
 * (`resolveDockEntries`), shown as a bar below the window area, where it reserves its space
 * (FR-003).
 */
import { computed } from 'vue'
import { resolveDockEntries } from '~/lib/wm/dock'

const wm = useWindowManagerStore()
const dock = useDock()

const entries = computed(() => resolveDockEntries(dock.items.value, wm.windows))
</script>

<template>
  <div
    class="flex shrink-0 justify-center p-2"
    data-testid="dock"
    :data-style="dock.placement.value.style"
    :data-edge="dock.placement.value.edge"
    :data-align="dock.placement.value.align"
  >
    <WmDockBar :entries="entries" orientation="horizontal" />
  </div>
</template>
