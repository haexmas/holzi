<script setup lang="ts">
/**
 * One entry in the list (spec 034, FR-001, FR-008, US1): icon, title (or the placeholder text of the
 * window for an entry without one), username, tag chips, a badge when it has expired and markers
 * for a TOTP and for passkeys. It holds a header only, never a secret.
 */
import type { ItemHeader } from '@bindings/ItemHeader'
import { DEFAULT_ENTRY_ICON, isKnownIcon } from '~/lib/passwords/icons'
import { displayTitle, isExpired, localDay } from '~/lib/passwords/format'

const props = defineProps<{
  header: ItemHeader
}>()

const { t } = useI18n()
const router = useTabRouter()

const title = computed(() => displayTitle(props.header.title))
const iconName = computed(() =>
  isKnownIcon(props.header.icon)
    ? (props.header.icon as string)
    : DEFAULT_ENTRY_ICON,
)
const expired = computed(() =>
  isExpired(props.header.expiresAt, localDay(new Date())),
)

function open() {
  router.push(`/entry/${props.header.id}`)
}
</script>

<template>
  <li>
    <button
      type="button"
      class="flex min-h-14 w-full items-center gap-4 px-4 py-3 text-left hover:bg-foreground/5"
      :data-testid="`passwords-entry-${header.id}`"
      @click="open"
    >
      <span
        class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-background"
        :style="header.color ? { color: header.color } : undefined"
        aria-hidden="true"
      >
        <Icon :name="iconName" class="size-5" />
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <span
          class="truncate"
          :class="title === null ? 'text-muted-foreground italic' : ''"
        >
          {{ title ?? t('passwords.untitled') }}
        </span>
        <span
          v-if="header.username"
          class="truncate text-sm text-muted-foreground"
        >
          {{ header.username }}
        </span>
      </span>
      <span class="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
        <ShadcnBadge v-if="expired" variant="destructive">
          {{ t('passwords.expired') }}
        </ShadcnBadge>
        <span
          v-if="header.hasTotp"
          class="text-muted-foreground"
          :title="t('passwords.markers.totp')"
        >
          <Icon name="lucide:timer" class="size-4" />
          <span class="sr-only">{{ t('passwords.markers.totp') }}</span>
        </span>
        <span
          v-if="header.passkeyCount > 0"
          class="text-muted-foreground"
          :title="t('passwords.markers.passkey')"
        >
          <Icon name="lucide:fingerprint" class="size-4" />
          <span class="sr-only">{{ t('passwords.markers.passkey') }}</span>
        </span>
        <ShadcnBadge
          v-for="tag in header.tags"
          :key="tag.id"
          variant="secondary"
          :style="tag.color ? { color: tag.color } : undefined"
        >
          {{ tag.name }}
        </ShadcnBadge>
      </span>
    </button>
  </li>
</template>
