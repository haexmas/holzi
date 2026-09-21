<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import type { EffortState } from '~/composables/useReasoningPreference'

type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string }[]
}

const MODEL_NAME_MAX_LENGTH = 20
/** Sentinel `ShadcnSelect` value for "no override" (`effortLevel: null`) —
 * Reka UI's Select works over strings, so `null` never appears on the wire
 * between this component and its select. */
const AUTO_VALUE = 'auto'

const props = defineProps<{
  modelId: string
  modelName?: string
  modelGroups: ModelGroup[]
  /** Options the displayed model offers, already labelled for display. Only
   * used while `effortState` is `selectable`. */
  effortChoices: { id: string; label: string }[]
  /** The effective option id; `null` is Auto. */
  effortLevel: string | null
  /** Trigger text for the current choice; empty when there is no active
   * choice to show (spec 012 FR-005). */
  effortLabel: string
  /** `hidden` renders nothing; `managed`/`unknown` render a disabled control
   * with a state label; `selectable` renders the options plus Auto. */
  effortState: EffortState
  disabled?: boolean
  modelDisabled?: boolean
}>()

const { t } = useI18n()

const emit = defineEmits<{
  'update:modelId': [modelId: string]
  'update:effortLevel': [effortLevel: string | null]
}>()

const root = ref<HTMLElement | null>(null)
const trigger = ref<HTMLButtonElement | null>(null)
const popover = ref<HTMLElement | null>(null)
const isOpen = ref(false)
const popoverStyle = ref<Record<string, string>>({})

const truncatedModelName = computed(() => {
  const name = props.modelName || t('chat.model.choose')
  const characters = Array.from(name)
  return characters.length > MODEL_NAME_MAX_LENGTH
    ? `${characters.slice(0, MODEL_NAME_MAX_LENGTH - 1).join('')}…`
    : name
})

function updateModel(value: unknown) {
  if (typeof value === 'string') emit('update:modelId', value)
}

function updateEffort(value: unknown) {
  if (typeof value !== 'string') return
  emit('update:effortLevel', value === AUTO_VALUE ? null : value)
}

function positionPopover() {
  if (!isOpen.value || !trigger.value || !popover.value) return

  const triggerRect = trigger.value.getBoundingClientRect()
  const popoverWidth = Math.min(352, window.innerWidth - 32)
  const left = Math.min(
    Math.max(16, triggerRect.left),
    Math.max(16, window.innerWidth - popoverWidth - 16),
  )

  popoverStyle.value = {
    left: `${left}px`,
    bottom: `${Math.max(16, window.innerHeight - triggerRect.top + 8)}px`,
    width: `${popoverWidth}px`,
  }
}

function openPopover() {
  if (props.disabled) return
  isOpen.value = true
  nextTick(() => requestAnimationFrame(positionPopover))
}

function closePopover(focusTrigger = false) {
  isOpen.value = false
  popoverStyle.value = {}
  if (focusTrigger) nextTick(() => trigger.value?.focus())
}

function togglePopover() {
  if (isOpen.value) closePopover()
  else openPopover()
}

/**
 * Reka's Select (and any other Popper-based reka component) teleports its
 * floating content to `document.body` on its own — a sibling of this
 * popover's own `<Teleport>`, not a descendant of `popover`/`root`.
 * Without this check, pointerdown on a dropdown item counted as
 * "outside," closing (and unmounting) this whole panel before the
 * Select's own click could commit the new value — the pick never reached
 * `updateModel`.
 */
function isInsideRekaPopperContent(target: Node): boolean {
  return (
    target instanceof Element &&
    target.closest('[data-reka-popper-content-wrapper]') !== null
  )
}

function closeOnOutsideClick(event: PointerEvent) {
  if (
    isOpen.value &&
    event.target instanceof Node &&
    !root.value?.contains(event.target) &&
    !popover.value?.contains(event.target) &&
    !isInsideRekaPopperContent(event.target)
  ) {
    closePopover()
  }
}

function closeOnEscape(event: KeyboardEvent) {
  if (event.key === 'Escape' && isOpen.value) {
    closePopover(true)
  }
}

function repositionPopover() {
  if (isOpen.value) positionPopover()
}

onMounted(() => {
  document.addEventListener('pointerdown', closeOnOutsideClick)
  document.addEventListener('keydown', closeOnEscape)
  window.addEventListener('resize', repositionPopover)
  window.addEventListener('scroll', repositionPopover, true)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOnOutsideClick)
  document.removeEventListener('keydown', closeOnEscape)
  window.removeEventListener('resize', repositionPopover)
  window.removeEventListener('scroll', repositionPopover, true)
})
</script>

<template>
  <div ref="root" class="shrink-0">
    <button
      ref="trigger"
      type="button"
      class="flex max-w-[min(17rem,calc(100vw-7rem))] cursor-pointer items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-muted-foreground outline-none transition-colors hover:bg-muted/60 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none"
      :class="{ 'pointer-events-none opacity-50': disabled }"
      :aria-label="
        [
          `${t('chat.composer.settingsButton')}: ${modelName ?? t('chat.model.choose')}`,
          effortLabel,
        ]
          .filter(Boolean)
          .join(', ')
      "
      :title="modelName || t('chat.model.choose')"
      :aria-expanded="isOpen"
      aria-haspopup="dialog"
      :disabled="disabled"
      @click="togglePopover"
    >
      <span class="max-w-[8rem] truncate sm:max-w-[11rem] lg:max-w-[15rem]">{{
        truncatedModelName
      }}</span>
      <template v-if="effortLabel">
        <span aria-hidden="true">·</span>
        <span class="shrink-0">{{ effortLabel }}</span>
      </template>
      <Icon
        name="lucide:chevron-down"
        class="h-3.5 w-3.5 shrink-0"
        :aria-hidden="true"
      />
    </button>

    <Teleport to="body">
      <div
        v-if="isOpen"
        id="chat-settings-popover"
        ref="popover"
        class="fixed z-50 max-h-[calc(100vh-2rem)] rounded-xl border border-border bg-popover p-3 text-popover-foreground shadow-lg"
        :style="popoverStyle"
        role="dialog"
        :aria-label="t('chat.composer.settingsPopover.title')"
      >
        <div class="mb-3 text-sm font-medium">
          {{ t('chat.composer.settingsPopover.title') }}
        </div>

        <label
          for="chat-model-popover"
          class="mb-1 block text-xs font-medium text-muted-foreground"
        >
          {{ t('chat.composer.settingsPopover.modelLabel') }}
        </label>
        <ShadcnSelect
          :model-value="modelId || undefined"
          :disabled="disabled || modelDisabled"
          @update:model-value="updateModel"
        >
          <ShadcnSelectTrigger
            id="chat-model-popover"
            :aria-label="t('chat.composer.settingsPopover.modelLabel')"
            class="mb-4 h-9 w-full bg-background text-sm"
          >
            <ShadcnSelectValue :placeholder="t('chat.model.choose')" />
          </ShadcnSelectTrigger>
          <ShadcnSelectContent
            class="w-[min(20rem,calc(100vw-2rem))] max-h-[60vh] !overflow-y-auto"
          >
            <ShadcnSelectGroup
              v-for="group in modelGroups"
              :key="group.providerId"
            >
              <ShadcnSelectLabel>{{ group.providerName }}</ShadcnSelectLabel>
              <ShadcnSelectItem
                v-for="model in group.models"
                :key="model.id"
                :value="model.id"
              >
                {{ model.name }}
              </ShadcnSelectItem>
            </ShadcnSelectGroup>
          </ShadcnSelectContent>
        </ShadcnSelect>

        <div v-if="effortState !== 'hidden'" class="mt-1">
          <label
            for="effort-level-popover"
            class="mb-1 block text-xs font-medium text-muted-foreground"
          >
            {{ t('chat.composer.settingsPopover.effortLabel') }}
          </label>
          <ShadcnSelect
            v-if="effortState === 'selectable'"
            :model-value="effortLevel ?? AUTO_VALUE"
            :disabled="disabled"
            @update:model-value="updateEffort"
          >
            <ShadcnSelectTrigger
              id="effort-level-popover"
              :aria-label="t('chat.composer.settingsPopover.effortLabel')"
              class="h-9 w-full bg-background text-sm"
            >
              <ShadcnSelectValue />
            </ShadcnSelectTrigger>
            <ShadcnSelectContent>
              <ShadcnSelectItem :value="AUTO_VALUE">
                {{ t('chat.effort.auto') }}
              </ShadcnSelectItem>
              <ShadcnSelectItem
                v-for="choice in effortChoices"
                :key="choice.id"
                :value="choice.id"
              >
                {{ choice.label }}
              </ShadcnSelectItem>
            </ShadcnSelectContent>
          </ShadcnSelect>
          <!-- Nothing to choose, but worth saying why: the model reasons on
               its own, or its capabilities are not known yet (spec 012). -->
          <ShadcnSelect v-else disabled>
            <ShadcnSelectTrigger
              id="effort-level-popover"
              :aria-label="t('chat.composer.settingsPopover.effortLabel')"
              class="h-9 w-full bg-background text-sm"
            >
              <ShadcnSelectValue
                :placeholder="
                  effortState === 'managed'
                    ? t('chat.effort.managed')
                    : t('chat.effort.unknown')
                "
              />
            </ShadcnSelectTrigger>
          </ShadcnSelect>
        </div>
      </div>
    </Teleport>
  </div>
</template>
