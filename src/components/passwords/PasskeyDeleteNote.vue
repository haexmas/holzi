<script setup lang="ts">
/**
 * In a dialog that deletes entries for good (spec 036, US5, T064): how many passkeys go with them.
 * Counts only an entry's own passkeys; one shown by a link stays with its source. Shows nothing
 * when there is none.
 */
const props = defineProps<{
  itemIds: readonly string[]
}>()

const { t } = useI18n()
const store = usePasswordsStore()

const count = computed(() =>
  props.itemIds.reduce(
    (sum, id) => sum + (store.headersById.get(id)?.passkeyCount ?? 0),
    0,
  ),
)
</script>

<template>
  <p
    v-if="count > 0"
    class="text-sm text-muted-foreground"
    data-testid="passwords-passkey-delete-note"
  >
    {{ t('passwords.passkeys.deletedWith', { count }, count) }}
  </p>
</template>
