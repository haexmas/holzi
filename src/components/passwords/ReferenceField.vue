<script setup lang="ts">
/**
 * The references of one editor field (spec 036, US7, research R11): a button "Verweis einfügen"
 * and, below the field, its marks as the backend reads them from the text (asked 200 ms after the
 * last keystroke) with a button that takes that placeholder out of the text. The input stays a
 * plain text field; the window never parses a placeholder itself. For a stored secret that is not
 * loaded (`text` is `null`) the marks come from the entry (`storedMarks`), and inserting replaces
 * the value.
 */
import type { RefMark } from '@bindings/RefMark'

const props = defineProps<{
  text: string | null
  /** The entry being edited, left out of the picker; `null` for a new one. */
  itemId: string | null
  /** Marks of the stored value while `text` is `null`. */
  storedMarks?: readonly RefMark[]
  /** Part of the test ids. */
  kind: string
}>()

const emit = defineEmits<{
  'update:text': [value: string]
}>()

const { t } = useI18n()
const { referencesParseAsync } = usePasswords()

const parsed = ref<RefMark[]>([])
/** The text `parsed` belongs to; its offsets fit no other text. */
const parsedFor = ref<string | null>(null)
const pickerOpen = ref(false)
let request = 0

async function parseAsync(text: string | null) {
  const mine = ++request
  if (text === null || !text.includes('{$')) {
    parsed.value = []
    parsedFor.value = text
    return
  }
  try {
    const marks = await referencesParseAsync(text)
    if (mine !== request) return
    parsed.value = marks
    parsedFor.value = text
  } catch {
    if (mine !== request) return
    parsed.value = []
    parsedFor.value = text
  }
}

watchDebounced(() => props.text, parseAsync, { debounce: 200 })
onMounted(() => void parseAsync(props.text))

const marks = computed(() =>
  props.text === null ? (props.storedMarks ?? []) : parsed.value,
)

/** Removing waits for the parse of the current text (200 ms debounce), or offsets cut wrongly. */
const canRemove = computed(
  () => props.text !== null && parsedFor.value === props.text,
)

/** UTF-16 offsets, as the backend reports them. */
function removeMark(mark: RefMark) {
  const text = props.text
  if (text === null || parsedFor.value !== text) return
  if (!text.startsWith('{$', mark.start)) return
  emit('update:text', text.slice(0, mark.start) + text.slice(mark.end))
}

function insert(token: string) {
  emit('update:text', props.text === null ? token : props.text + token)
}
</script>

<template>
  <div
    class="flex flex-wrap items-center gap-1.5"
    :data-testid="`passwords-reference-field-${kind}`"
  >
    <template v-for="mark in marks" :key="`${mark.start}-${mark.sourceItemId}`">
      <span class="inline-flex items-center gap-0.5">
        <PasswordsReferenceValue
          :marks="[mark]"
          :text="null"
          :kind="kind"
          :navigable="false"
        />
        <UiButton
          v-if="text !== null"
          type="button"
          :disabled="!canRemove"
          variant="ghost"
          size="icon-sm"
          class="size-6"
          :aria-label="t('passwords.references.remove')"
          :data-testid="`passwords-reference-remove-${kind}`"
          @click="removeMark(mark)"
        >
          <Icon name="lucide:x" class="size-3" />
        </UiButton>
      </span>
    </template>
    <UiButton
      type="button"
      variant="ghost"
      size="sm"
      class="h-7 px-2 text-xs"
      :data-testid="`passwords-reference-insert-${kind}`"
      @click="pickerOpen = true"
    >
      <Icon name="lucide:link" class="size-3.5" />
      {{ t('passwords.references.insert') }}
    </UiButton>
    <PasswordsReferencePicker
      v-model:open="pickerOpen"
      :item-id="itemId"
      @pick="insert"
    />
  </div>
</template>
