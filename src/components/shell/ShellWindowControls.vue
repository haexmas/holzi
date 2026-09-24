<script setup lang="ts">
/** Minimize/maximize-restore/close button group (spec 015-workspace-shell, T029/T037, FR-007's
 * right-hand title-bar order). Compact mode hides minimize/maximize — FR-036 offers neither there
 * (a compact window already fills the area and cannot be un-focused onto a visible desktop). */
defineProps<{
  maximized: boolean
  compact: boolean
}>()

const emit = defineEmits<{
  minimize: []
  toggleMaximize: []
  close: []
}>()

const { t } = useI18n()
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5" @pointerdown.stop>
    <template v-if="!compact">
      <button
        type="button"
        class="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        :aria-label="t('shell.window.minimize')"
        @click="emit('minimize')"
      >
        <Icon name="lucide:minus" class="h-3.5 w-3.5" :aria-hidden="true" />
      </button>
      <button
        type="button"
        class="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        :aria-label="
          maximized ? t('shell.window.restore') : t('shell.window.maximize')
        "
        @click="emit('toggleMaximize')"
      >
        <Icon
          :name="maximized ? 'lucide:minimize-2' : 'lucide:maximize-2'"
          class="h-3.5 w-3.5"
          :aria-hidden="true"
        />
      </button>
    </template>
    <button
      type="button"
      class="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
      :aria-label="t('shell.window.close')"
      @click="emit('close')"
    >
      <Icon name="lucide:x" class="h-3.5 w-3.5" :aria-hidden="true" />
    </button>
  </div>
</template>
