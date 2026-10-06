<script setup lang="ts">
/**
 * The swipe surface of the entry tabs (spec 036, FR-002, FR-003, research R1): one slide per tab,
 * the height follows the slide that shows. Finger, pen and mouse drag, and a horizontal touchpad
 * swipe changes the tab too; inputs and anything marked `data-no-swipe` (values one selects) keep
 * their own gestures.
 * The tab bar above is `EntryTabs.vue`; this part is loaded on demand so the list starts without
 * the library.
 */
import { useMediaQuery } from '@vueuse/core'
import type { Swiper as SwiperInstance } from 'swiper/types'
import { Mousewheel } from 'swiper/modules'
import { Swiper, SwiperSlide } from 'swiper/vue'
import 'swiper/css'
import type { EntryTab } from '~/lib/passwords/registry'

const props = defineProps<{
  tabs: readonly EntryTab[]
  modelValue: EntryTab
}>()
const emit = defineEmits<{ 'update:modelValue': [tab: EntryTab] }>()

const reducedMotion = useMediaQuery('(prefers-reduced-motion: reduce)')
const root = ref<HTMLElement | null>(null)
const swiper = shallowRef<SwiperInstance | null>(null)
const index = computed(() => Math.max(0, props.tabs.indexOf(props.modelValue)))

/** What keeps its own gestures: no swipe and no touchpad swipe starts there (FR-003). */
const NO_SWIPE = 'input, textarea, select, [data-no-swipe], .swiper-no-swiping'

// Swiper's mousewheel module only knows its own class, not `noSwipingSelector`: a horizontal
// touchpad scroll over a zone that must not swipe (the history timeline) would change the tab.
// Such a wheel event stops here, before it reaches Swiper; the zone still scrolls natively.
function keepWheel(event: WheelEvent) {
  if (event.target instanceof Element && event.target.closest(NO_SWIPE)) {
    event.stopPropagation()
  }
}

function onSwiper(instance: SwiperInstance) {
  swiper.value = instance
}

function onSlideChange(instance: SwiperInstance) {
  const tab = props.tabs[instance.activeIndex]
  if (tab && tab !== props.modelValue) emit('update:modelValue', tab)
}

// A tap on the tab bar (or a place change) moves the slide.
watch(index, (next) => {
  const current = swiper.value
  if (current && current.activeIndex !== next)
    current.slideTo(next, reducedMotion.value ? 0 : undefined)
})

// The wrapper takes the height of the shown slide; a slide whose content changes (a load, a
// reveal) has to tell it.
let observer: ResizeObserver | null = null
onMounted(() => {
  if (typeof ResizeObserver === 'undefined') return
  observer = new ResizeObserver(() => swiper.value?.updateAutoHeight(0))
  root.value
    ?.querySelectorAll('.swiper-slide')
    .forEach((slide) => observer?.observe(slide))
})
onBeforeUnmount(() => observer?.disconnect())
</script>

<template>
  <div ref="root" @wheel.capture.passive="keepWheel">
    <Swiper
      :slides-per-view="1"
      :auto-height="true"
      :initial-slide="index"
      :modules="[Mousewheel]"
      :mousewheel="{ forceToAxis: true, thresholdDelta: 10 }"
      :threshold="8"
      :no-swiping="true"
      :no-swiping-selector="NO_SWIPE"
      :touch-start-prevent-default="false"
      :resistance="false"
      :speed="reducedMotion ? 0 : 300"
      data-testid="entry-tabs-swiper"
      @swiper="onSwiper"
      @slide-change="onSlideChange"
    >
      <SwiperSlide
        v-for="tab in tabs"
        :key="tab"
        role="tabpanel"
        :data-testid="`entry-panel-${tab}`"
      >
        <slot :name="tab" />
      </SwiperSlide>
    </Swiper>
  </div>
</template>
