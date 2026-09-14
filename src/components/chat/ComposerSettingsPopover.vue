<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

type EffortLevel = 'low' | 'medium' | 'high'

type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string, name: string }[]
}

const props = defineProps<{
  modelId: string
  modelName?: string
  modelGroups: ModelGroup[]
  effortLevel: EffortLevel
  effortLabel: string
  disabled?: boolean
  modelDisabled?: boolean
}>()

const { t } = useI18n()

const emit = defineEmits<{
  'update:modelId': [modelId: string]
  'update:effortLevel': [effortLevel: EffortLevel]
}>()

const root = ref<HTMLDetailsElement | null>(null)
const effortLevels: EffortLevel[] = ['low', 'medium', 'high']

const effortIndex = computed(() => effortLevels.indexOf(props.effortLevel))

function updateEffort(value: number) {
  const level = effortLevels[Math.max(0, Math.min(effortLevels.length - 1, Math.round(value)))]
  if (level) emit('update:effortLevel', level)
}

function closeOnOutsideClick(event: PointerEvent) {
  if (root.value?.open && event.target instanceof Node && !root.value.contains(event.target)) {
    root.value.open = false
  }
}

function closeOnEscape(event: KeyboardEvent) {
  if (event.key === 'Escape' && root.value?.open) {
    root.value.open = false
    root.value.querySelector<HTMLElement>('summary')?.focus()
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', closeOnOutsideClick)
  document.addEventListener('keydown', closeOnEscape)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOnOutsideClick)
  document.removeEventListener('keydown', closeOnEscape)
})
</script>

<template>
  <details ref="root" class="relative shrink-0">
    <summary
      class="flex max-w-[min(19rem,calc(100vw-7rem))] cursor-pointer list-none items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-muted-foreground outline-none transition-colors hover:bg-muted/60 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none"
      :class="{ 'pointer-events-none opacity-50': disabled }"
      :aria-label="`${t('chat.composer.settingsButton')}: ${modelName ?? t('chat.model.choose')}, ${effortLabel}`"
      :aria-disabled="disabled || undefined"
      @click="disabled && $event.preventDefault()"
    >
      <Icon name="lucide:sliders-horizontal" class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
      <span class="truncate">{{ modelName || t('chat.model.choose') }}</span>
      <span aria-hidden="true">·</span>
      <span class="shrink-0">{{ effortLabel }}</span>
      <Icon name="lucide:chevron-down" class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
    </summary>

    <div
      class="absolute bottom-full left-0 z-30 mb-2 w-[min(22rem,calc(100vw-2rem))] rounded-xl border border-border bg-popover p-3 text-popover-foreground shadow-lg"
      role="dialog"
      :aria-label="t('chat.composer.settingsPopover.title')"
    >
      <div class="mb-3 text-sm font-medium">
        {{ t('chat.composer.settingsPopover.title') }}
      </div>

      <label for="chat-model-popover" class="mb-1 block text-xs font-medium text-muted-foreground">
        {{ t('chat.composer.settingsPopover.modelLabel') }}
      </label>
      <select
        id="chat-model-popover"
        class="mb-4 w-full rounded-lg border border-border bg-background px-2.5 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
        :value="modelId"
        :disabled="disabled || modelDisabled"
        :aria-label="t('chat.composer.settingsPopover.modelLabel')"
        @change="emit('update:modelId', ($event.target as HTMLSelectElement).value)"
      >
        <option value="" disabled>{{ t('chat.model.choose') }}</option>
        <optgroup v-for="group in modelGroups" :key="group.providerId" :label="group.providerName">
          <option v-for="model in group.models" :key="model.id" :value="model.id">
            {{ model.name }}
          </option>
        </optgroup>
      </select>

      <div class="flex items-center justify-between gap-3">
        <label for="effort-level-popover" class="text-xs font-medium text-muted-foreground">
          {{ t('chat.composer.settingsPopover.effortLabel') }}
        </label>
        <output for="effort-level-popover" class="text-sm font-medium">
          {{ effortLabel }}
        </output>
      </div>
      <input
        id="effort-level-popover"
        class="mt-2 w-full accent-foreground disabled:opacity-50"
        type="range"
        min="0"
        max="2"
        step="1"
        :value="effortIndex"
        :disabled="disabled"
        :aria-label="t('chat.composer.settingsPopover.effortLabel')"
        :aria-valuetext="effortLabel"
        @input="updateEffort(Number(($event.target as HTMLInputElement).value))"
      >
      <div class="mt-1 flex justify-between text-[10px] text-muted-foreground" aria-hidden="true">
        <span>{{ t('chat.effort.low') }}</span>
        <span>{{ t('chat.effort.medium') }}</span>
        <span>{{ t('chat.effort.high') }}</span>
      </div>
    </div>
  </details>
</template>
