<script setup lang="ts">
/**
 * The password manager frame (spec 034-password-manager, research R13), laid out like the chat and
 * settings frames: a toolbar row with the sidebar button, below it the sidebar and the content.
 * The frame is a size container, so the sidebar follows the window's width, not the screen's: from
 * `@2xl` it sits beside the content and can be hidden, below it is gone and opens over the content
 * (`useSidebarFrame`). Every place is a tab location (spec 020); only ids go into the path.
 */
const { t } = useI18n()

const root = useTemplateRef<HTMLElement>('root')
const sidebar = useTemplateRef<HTMLElement>('sidebar')

const { wideHidden, menuOpen, visible, overlaying, toggle, close } =
  useSidebarFrame(root, () => sidebar.value)

const label = computed(() =>
  visible.value ? t('passwords.sidebar.hide') : t('passwords.sidebar.show'),
)

const sidebarClass = computed(() => [
  'absolute inset-0 z-20 bg-background transition-[translate,width,visibility] duration-200 ease-out motion-reduce:transition-none',
  '@2xl:static @2xl:z-auto @2xl:shrink-0 @2xl:overflow-hidden @2xl:translate-x-0',
  menuOpen.value ? 'visible translate-x-0' : 'invisible -translate-x-full',
  wideHidden.value ? '@2xl:invisible @2xl:w-0' : '@2xl:visible @2xl:w-64',
])

function onEscape() {
  if (!overlaying.value) return
  close()
  root.value
    ?.querySelector<HTMLElement>('[data-testid="passwords-sidebar-toggle"]')
    ?.focus()
}
</script>

<template>
  <div ref="root" class="@container flex h-full min-h-0 flex-col bg-muted/20">
    <div class="flex h-13 shrink-0 items-center gap-2 px-2">
      <UiButton
        variant="ghost"
        size="icon"
        :aria-label="label"
        :tooltip="label"
        :aria-expanded="visible"
        aria-controls="passwords-sidebar"
        data-testid="passwords-sidebar-toggle"
        @keydown.esc="onEscape"
        @click="toggle"
      >
        <Icon name="lucide:panel-left" class="size-4" />
      </UiButton>
    </div>
    <div class="relative flex min-h-0 flex-1 overflow-hidden">
      <nav
        id="passwords-sidebar"
        ref="sidebar"
        :class="sidebarClass"
        :aria-label="t('passwords.title')"
        data-testid="passwords-sidebar"
        @keydown.esc="onEscape"
      />
      <main
        class="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto"
        :inert="overlaying || undefined"
      >
        <WmRouterView />
      </main>
    </div>
  </div>
</template>
