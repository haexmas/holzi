<script setup lang="ts">
/**
 * The chat frame: a toolbar row with only the sidebar button, below it the thread sidebar and the
 * content. Like the settings frame (`apps/SettingsApp.vue`) it is a size container, so the sidebar
 * follows the window's width, not the screen's: from `@2xl` it sits beside the content and can be
 * hidden, below that it is gone and opens over the content; the width logic is
 * `useSidebarFrame`. The `sidebar` slot gets `run`, to wrap what a pick in the overlay does.
 */
const { t } = useI18n()

const frame = useTemplateRef<HTMLElement>('frame')
const panel = useTemplateRef<HTMLElement>('panel')

const { wideHidden, menuOpen, visible, overlaying, toggle, close } =
  useSidebarFrame(frame, () => panel.value)

const label = computed(() =>
  visible.value ? t('chat.sidebar.hide') : t('chat.sidebar.show'),
)

const panelClass = computed(() => [
  'absolute inset-0 z-20 bg-background transition-[translate,width,visibility] duration-200 ease-out motion-reduce:transition-none',
  '@2xl:static @2xl:z-auto @2xl:shrink-0 @2xl:overflow-hidden @2xl:translate-x-0',
  menuOpen.value ? 'visible translate-x-0' : 'invisible -translate-x-full',
  wideHidden.value ? '@2xl:invisible @2xl:w-0' : '@2xl:visible @2xl:w-64',
])

/** For what is picked in the sidebar: the overlay closes, back to the content; a wide window keeps
 * its sidebar. */
function run(action: () => void) {
  close()
  action()
}

function onEscape() {
  if (!overlaying.value) return
  close()
  frame.value
    ?.querySelector<HTMLElement>('[data-testid="chat-sidebar-toggle"]')
    ?.focus()
}
</script>

<template>
  <main ref="frame" class="@container flex h-full min-h-0 flex-col bg-muted/20">
    <div class="flex h-13 shrink-0 items-center px-2">
      <UiButton
        variant="ghost"
        size="icon"
        :aria-label="label"
        :tooltip="label"
        :aria-expanded="visible"
        aria-controls="chat-sidebar"
        data-testid="chat-sidebar-toggle"
        @keydown.esc="onEscape"
        @click="toggle"
      >
        <Icon name="lucide:panel-left" class="size-4" />
      </UiButton>
    </div>
    <div class="relative flex min-h-0 flex-1 overflow-hidden">
      <div
        id="chat-sidebar"
        ref="panel"
        :class="panelClass"
        @keydown.esc="onEscape"
      >
        <slot name="sidebar" :run="run" />
      </div>
      <div
        class="flex min-h-0 min-w-0 flex-1 flex-col"
        :inert="overlaying || undefined"
      >
        <slot />
      </div>
    </div>
    <slot name="dialogs" />
  </main>
</template>
