<script setup lang="ts">
/**
 * The right-click menu of a row, a folder, the empty area or the trash (spec 036, US4, FR-018): it
 * renders the list of `lib/passwords/menus.ts` around its slot and reports the chosen command; the
 * menu button of a row (`EntryMenuButton.vue`) shows the same list where there is no right button.
 */
import type { MenuCommand, MenuEntry } from '~/lib/passwords/menus'

defineProps<{
  entries: readonly MenuEntry[]
}>()

const emit = defineEmits<{
  run: [command: MenuCommand]
  /** The menu opens: the row decides what it applies to before the actions show. */
  open: []
}>()

const { t } = useI18n()
const { shortcut } = usePasswordsMenuText()
</script>

<template>
  <ShadcnContextMenu @update:open="(open: boolean) => open && emit('open')">
    <ShadcnContextMenuTrigger as-child>
      <slot />
    </ShadcnContextMenuTrigger>
    <ShadcnContextMenuContent
      class="min-w-52"
      data-testid="passwords-context-menu"
    >
      <template v-for="(entry, index) in entries" :key="index">
        <ShadcnContextMenuSeparator v-if="'separator' in entry" />
        <ShadcnContextMenuItem
          v-else
          :disabled="entry.disabled"
          :data-testid="`passwords-menu-${entry.id}`"
          @select="emit('run', entry.id)"
        >
          {{ t(entry.labelKey) }}
          <ShadcnContextMenuShortcut v-if="entry.shortcut">
            {{ shortcut(entry.shortcut) }}
          </ShadcnContextMenuShortcut>
        </ShadcnContextMenuItem>
      </template>
    </ShadcnContextMenuContent>
  </ShadcnContextMenu>
</template>
