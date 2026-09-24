<script setup lang="ts">
/**
 * Page header (active thread title, model status dot/name, mobile
 * settings/lock/new-chat shortcuts) — extracted from
 * `src/pages/chat/[instance].vue` (spec 015-workspace-shell, T013, to keep
 * the orchestrator page under the 500-line constitution limit).
 * Presentational only.
 */
defineProps<{
  instanceName: string
  title: string
  modelLoaded: boolean
  modelName: string
  busy: boolean
}>()

const emit = defineEmits<{
  lock: []
  newChat: []
}>()

const { t } = useI18n()
</script>

<template>
  <header
    class="flex items-center justify-between gap-3 border-b border-border bg-background/90 px-4 py-3 backdrop-blur md:px-6"
  >
    <div class="min-w-0">
      <div class="flex items-center gap-2">
        <div
          class="h-2 w-2 rounded-full"
          :class="modelLoaded ? 'bg-emerald-500' : 'bg-muted-foreground/40'"
        />
        <h1 class="truncate text-sm font-semibold">{{ title }}</h1>
      </div>
      <p class="mt-0.5 truncate text-xs text-muted-foreground">
        {{ modelName }}
      </p>
    </div>
    <div class="flex shrink-0 items-center gap-1 md:hidden">
      <NuxtLink
        :to="`/settings/${encodeURIComponent(instanceName)}`"
        class="rounded-lg p-2 text-muted-foreground hover:bg-accent hover:text-foreground"
        :aria-label="t('chat.settings')"
      >
        <Icon name="lucide:settings-2" class="h-4 w-4" />
      </NuxtLink>
      <UiButton
        size="sm"
        variant="ghost"
        data-testid="lock-instance-header"
        :aria-label="t('chat.lock')"
        @click="emit('lock')"
      >
        <Icon name="lucide:lock-keyhole" class="h-4 w-4" />
      </UiButton>
      <UiButton
        class="gap-2"
        size="sm"
        variant="outline"
        :disabled="busy"
        @click="emit('newChat')"
      >
        <Icon name="lucide:plus" class="h-4 w-4" />
        {{ t('chat.newChatShort') }}
      </UiButton>
    </div>
  </header>
</template>
