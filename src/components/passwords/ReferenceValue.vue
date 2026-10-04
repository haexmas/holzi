<script setup lang="ts">
/**
 * A field value that holds references (spec 036, US7, FR-044 to FR-047, research R11): the
 * placeholders show as marks "Passwort von Konto" (a tap opens the source), the text around them
 * as text. For a secret field the text is not known here (`text` is `null`) and only the marks
 * show; the value itself is revealed or copied through the backend, which resolves it. A mark whose
 * source is gone or that loops says so instead of the value.
 */
import type { RefMark } from '@bindings/RefMark'
import { displayTitle } from '~/lib/passwords/format'

const props = defineProps<{
  marks: readonly RefMark[]
  /** The raw text of a field that is no secret; `null` shows the marks alone. */
  text?: string | null
  /** Part of the test ids. */
  kind: string
}>()

const { t } = useI18n()
const router = useTabRouter()

type Part = { kind: 'text'; text: string } | { kind: 'mark'; mark: RefMark }

const parts = computed<Part[]>(() => {
  const text = props.text
  if (text === null || text === undefined) {
    return props.marks.map((mark) => ({ kind: 'mark', mark }))
  }
  const out: Part[] = []
  let last = 0
  for (const mark of props.marks) {
    if (mark.start > last)
      out.push({ kind: 'text', text: text.slice(last, mark.start) })
    out.push({ kind: 'mark', mark })
    last = mark.end
  }
  if (last < text.length) out.push({ kind: 'text', text: text.slice(last) })
  return out
})

function label(mark: RefMark): string {
  const value =
    mark.kind === 'extra'
      ? t('passwords.references.kind.extra', { key: mark.key ?? '' })
      : t(`passwords.references.kind.${mark.kind}`)
  const source = displayTitle(mark.sourceTitle) ?? t('passwords.untitled')
  return t('passwords.references.mark', { value, source })
}

function problem(mark: RefMark): string | null {
  if (mark.status === 'ok') return null
  return mark.status === 'missing'
    ? t('passwords.references.status.missing')
    : t('passwords.references.status.cycle')
}

function open(mark: RefMark) {
  if (mark.status !== 'missing') router.push(`/entry/${mark.sourceItemId}`)
}
</script>

<template>
  <span
    class="flex min-w-0 flex-wrap items-center gap-1 text-sm"
    :data-testid="`passwords-references-${kind}`"
  >
    <template v-for="(part, index) in parts" :key="index">
      <span v-if="part.kind === 'text'" class="whitespace-pre-wrap">{{
        part.text
      }}</span>
      <button
        v-else
        type="button"
        class="inline-flex max-w-full items-center gap-1 rounded-full border px-2 py-0.5 text-xs"
        :class="
          part.mark.status === 'ok'
            ? 'border-primary/40 bg-primary/10 hover:bg-primary/20'
            : 'border-destructive/50 bg-destructive/10 text-destructive'
        "
        :title="problem(part.mark) ?? t('passwords.references.open')"
        :disabled="part.mark.status === 'missing'"
        :data-testid="`passwords-reference-mark-${part.mark.status}`"
        @click="open(part.mark)"
      >
        <Icon
          :name="
            part.mark.status === 'ok' ? 'lucide:link' : 'lucide:triangle-alert'
          "
          class="size-3 shrink-0"
        />
        <span class="truncate">{{ label(part.mark) }}</span>
        <span v-if="problem(part.mark)" class="shrink-0"
          >· {{ problem(part.mark) }}</span
        >
      </button>
    </template>
  </span>
</template>
