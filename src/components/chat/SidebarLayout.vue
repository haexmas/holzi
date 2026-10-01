<script setup lang="ts">
/**
 * The chat frame: a toolbar row with only the sidebar button, below it the thread sidebar and the
 * content. Like the settings frame (`apps/SettingsApp.vue`) it is a size container, so the sidebar
 * follows the window's width, not the screen's: from `@2xl` it sits beside the content and can be
 * hidden, below that it is gone and opens over the content. Nothing is kept — the state is local
 * to the open tab. Whether the frame is wide is read from the panel's CSS (`position`) rather
 * than a second copy of the threshold. The `sidebar` slot gets `run`, to wrap what a pick in the
 * overlay does.
 */
const { t } = useI18n()

const frame = useTemplateRef<HTMLElement>('frame')
const panel = useTemplateRef<HTMLElement>('panel')

/** Wide window: the operator hid the sidebar. */
const wideHidden = ref(false)
/** Narrow window: the sidebar is open over the content. */
const menuOpen = ref(false)
const wide = ref(true)

function isWide(): boolean {
  const el = panel.value
  return el ? getComputedStyle(el).position !== 'absolute' : true
}

const { width } = useElementSize(frame)
watch(width, () => {
  wide.value = isWide()
  if (wide.value) menuOpen.value = false
})
onMounted(() => {
  wide.value = isWide()
})

const visible = computed(() =>
  wide.value ? !wideHidden.value : menuOpen.value,
)
/** The sidebar covers the content, which must not take focus meanwhile. */
const overlaying = computed(() => menuOpen.value && !wide.value)
const label = computed(() =>
  visible.value ? t('chat.sidebar.hide') : t('chat.sidebar.show'),
)

const panelClass = computed(() => [
  'absolute inset-0 z-20 bg-background transition-[translate,width,visibility] duration-200 ease-out motion-reduce:transition-none',
  '@2xl:static @2xl:z-auto @2xl:shrink-0 @2xl:overflow-hidden @2xl:translate-x-0',
  menuOpen.value ? 'visible translate-x-0' : 'invisible -translate-x-full',
  wideHidden.value ? '@2xl:invisible @2xl:w-0' : '@2xl:visible @2xl:w-64',
])

function toggle() {
  if (isWide()) wideHidden.value = visible.value
  else menuOpen.value = !menuOpen.value
}

function close() {
  menuOpen.value = false
}

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
