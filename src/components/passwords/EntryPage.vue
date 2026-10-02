<script setup lang="ts">
/**
 * The place `/entry/:id` (spec 034, research R13): the entry's view, or its editor for
 * `/entry/:id?edit` and for `/entry/new`. The path carries the id only, never a title or a value.
 */
const router = useTabRouter()

const itemId = computed(() => router.route.params.id ?? '')
const isNew = computed(() => itemId.value === 'new')
const editing = computed(() => router.route.query.edit !== undefined)
</script>

<template>
  <PasswordsEntryEditor
    v-if="isNew || editing"
    :key="isNew ? 'new' : `edit-${itemId}`"
    :item-id="isNew ? null : itemId"
  />
  <PasswordsEntryView v-else :key="itemId" :item-id="itemId" />
</template>
