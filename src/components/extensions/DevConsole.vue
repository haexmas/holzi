<script setup lang="ts">
/**
 * The console output of a development version (spec 017, US12, FR-045): what its page forwards
 * through the SDK, newest at the bottom. Folded by default so the frame keeps its room.
 */
import type { DevConsoleLine } from '~/lib/extensions/devConsole'

const props = defineProps<{ lines: readonly DevConsoleLine[] }>()
const { t } = useI18n()
const open = ref(false)
const list = ref<HTMLElement | null>(null)

watch(
  () => props.lines.length,
  async () => {
    if (!open.value) return
    await nextTick()
    list.value?.scrollTo({ top: list.value.scrollHeight })
  },
)
</script>

<template>
  <section class="border-t border-border bg-muted/40" data-testid="dev-console">
    <button
      type="button"
      class="flex w-full items-center gap-2 px-3 py-1 text-xs text-muted-foreground hover:bg-accent"
      :aria-expanded="open"
      data-testid="dev-console-toggle"
      @click="open = !open"
    >
      <Icon
        :name="open ? 'lucide:chevron-down' : 'lucide:chevron-right'"
        class="h-3 w-3"
        :aria-hidden="true"
      />
      {{ t('extensions.dev.console', { count: lines.length }) }}
    </button>
    <ol
      v-if="open"
      ref="list"
      class="max-h-48 overflow-auto px-3 pb-2 font-mono text-xs"
      aria-live="polite"
    >
      <li
        v-for="(line, index) in lines"
        :key="index"
        class="break-words whitespace-pre-wrap"
        :class="{
          'text-destructive': line.level === 'error',
          'text-warning': line.level === 'warn',
          'text-muted-foreground': line.level === 'debug',
        }"
        data-testid="dev-console-line"
      >
        <span class="text-muted-foreground">{{ line.time }}</span>
        {{ line.message }}
      </li>
    </ol>
  </section>
</template>
