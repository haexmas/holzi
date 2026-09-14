import { nextTick, onMounted, ref, watch, type Ref } from 'vue'

type AutoResizeOptions = {
  minHeight?: number
  maxHeight?: number
}

/** Keeps a textarea as small as possible while bounding it for long prompts. */
export function useAutoResizeTextarea(
  value: Ref<string>,
  options: AutoResizeOptions = {},
) {
  const textareaRef = ref<HTMLTextAreaElement | null>(null)
  const isOverflowing = ref(false)
  const minHeight = options.minHeight ?? 44
  const maxHeight = options.maxHeight ?? 212

  function resize() {
    const textarea = textareaRef.value
    if (!textarea) return
    textarea.style.height = 'auto'
    const height = Math.min(Math.max(textarea.scrollHeight, minHeight), maxHeight)
    isOverflowing.value = textarea.scrollHeight > maxHeight
    textarea.style.height = `${height}px`
    textarea.style.overflowY = isOverflowing.value ? 'auto' : 'hidden'
  }

  async function reset() {
    await nextTick()
    resize()
  }

  watch(value, () => {
    void nextTick(resize)
  })
  onMounted(resize)

  return {
    textareaRef,
    isOverflowing,
    resize,
    reset,
  }
}
