<script setup lang="ts">
const { t } = useI18n()

const props = defineProps<{
  instance: string
}>()

const target = computed(() => `/chat/${encodeURIComponent(props.instance)}`)
</script>

<template>
  <NuxtLink
    :to="target"
    class="fixed bottom-6 right-6 h-14 w-14 rounded-full shadow-lg flex items-center justify-center bg-blue-600 text-white hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
    :title="t('workspace.chatFab.openTitle')"
    :aria-label="t('workspace.chatFab.openTitle')"
  >
    <Icon name="lucide:message-circle" class="h-6 w-6" />
  </NuxtLink>
</template>
