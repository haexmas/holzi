<script setup lang="ts">
/**
 * The right-click menu of the open folder (a long press on touch; spec 044 T050): one menu around
 * the whole list, its entries from `lib/files/menus.ts` for whatever the pointer was over.
 */
import type { FilesCommand, FilesMenuEntry } from '~/lib/files/menus'

defineProps<{ entries: readonly FilesMenuEntry[] }>()
const emit = defineEmits<{ run: [command: FilesCommand] }>()

const { t } = useI18n()
</script>

<template>
  <ShadcnContextMenu>
    <ShadcnContextMenuTrigger as-child>
      <slot />
    </ShadcnContextMenuTrigger>
    <ShadcnContextMenuContent class="min-w-48" data-testid="files-context-menu">
      <template v-for="(entry, index) in entries" :key="index">
        <ShadcnContextMenuSeparator v-if="'separator' in entry" />
        <ShadcnContextMenuItem
          v-else
          :disabled="entry.disabled"
          :data-testid="`files-menu-${entry.id}`"
          @select="emit('run', entry.id)"
        >
          {{ t(entry.labelKey) }}
        </ShadcnContextMenuItem>
      </template>
    </ShadcnContextMenuContent>
  </ShadcnContextMenu>
</template>
