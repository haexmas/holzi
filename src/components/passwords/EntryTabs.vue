<script setup lang="ts">
/**
 * The tabs of an entry (spec 036, FR-001, FR-002, FR-006, research R1): a tab bar and, below it,
 * one slide per tab that follows the finger. Tapping a tab and the arrow keys of the bar come from
 * the tab component; a swipe comes from `EntryTabsSwiper.vue`. The chosen tab is the value of
 * `v-model`; the parent keeps it in the place of the window tab (`registry.ts` `entryTab`).
 */
import type { EntryTab } from '~/lib/passwords/registry'

defineProps<{
  tabs: readonly EntryTab[]
  modelValue: EntryTab
}>()
const emit = defineEmits<{ 'update:modelValue': [tab: EntryTab] }>()

const { t } = useI18n()
const Swiper = defineAsyncComponent(() => import('./EntryTabsSwiper.vue'))
</script>

<template>
  <div class="flex flex-col gap-3" data-testid="entry-tabs">
    <ShadcnTabs
      :model-value="modelValue"
      @update:model-value="emit('update:modelValue', $event as EntryTab)"
    >
      <ShadcnTabsList class="w-full" :aria-label="t('passwords.tabs.label')">
        <ShadcnTabsTrigger
          v-for="tab in tabs"
          :key="tab"
          :value="tab"
          :data-testid="`entry-tab-${tab}`"
        >
          {{ t(`passwords.tabs.${tab}`) }}
        </ShadcnTabsTrigger>
      </ShadcnTabsList>
    </ShadcnTabs>
    <Swiper
      :tabs="tabs"
      :model-value="modelValue"
      @update:model-value="emit('update:modelValue', $event)"
    >
      <template v-for="tab in tabs" :key="tab" #[tab]>
        <slot :name="tab" />
      </template>
    </Swiper>
  </div>
</template>
