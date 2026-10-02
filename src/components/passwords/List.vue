<script setup lang="ts">
/**
 * The list of entries (spec 034, US1, FR-007): every entry outside the trash, filtered by the
 * search text of the place (`?q=`), in the boxed list style of the other holzi views. While the
 * overview has not been loaded once it shows a spinner instead of the empty state.
 */
import { filterHeaders, fold } from '~/lib/passwords/search'
import { isInTrash, trashGroupIds } from '~/lib/passwords/tree'

const { t } = useI18n()
const router = useTabRouter()
const store = usePasswordsStore()

const visible = computed(() => {
  const trash = trashGroupIds(store.groups)
  const live = store.headers.filter(
    (header) => !isInTrash(header.groupId, trash),
  )
  const found = filterHeaders(live, {
    query: router.route.query.q ?? '',
    tagId: router.route.query.tag,
  })
  return [...found].sort((a, b) => {
    const left = fold(a.title ?? '')
    const right = fold(b.title ?? '')
    // Entries without a title come last, then by title and id.
    if (!left !== !right) return left ? -1 : 1
    return left.localeCompare(right) || a.id.localeCompare(b.id)
  })
})

const searching = computed(() => Boolean(router.route.query.q))
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-6 @md:px-6">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-3">
      <h1 class="px-1 pt-1 text-2xl font-bold" data-testid="passwords-title">
        {{ t('passwords.title') }}
      </h1>
      <p v-if="store.lastError" class="text-sm text-destructive" role="alert">
        {{ store.lastError }}
      </p>
      <div
        v-if="!store.hasLoadedOnce"
        class="flex justify-center py-10 text-muted-foreground"
        role="status"
        data-testid="passwords-loading"
      >
        <Icon name="lucide:loader-circle" class="size-6 animate-spin" />
        <span class="sr-only">{{ t('passwords.loading') }}</span>
      </div>
      <p
        v-else-if="visible.length === 0"
        class="py-10 text-center text-muted-foreground"
        data-testid="passwords-empty"
      >
        {{ searching ? t('passwords.noMatches') : t('passwords.empty') }}
      </p>
      <SettingsGroup v-else>
        <PasswordsListItem
          v-for="header in visible"
          :key="header.id"
          :header="header"
        />
      </SettingsGroup>
    </div>
  </div>
</template>
