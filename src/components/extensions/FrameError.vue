<script setup lang="ts">
/** Shown instead of an extension frame that could not start or did not answer (spec 017, T046). */
defineProps<{ message: string }>()
const emit = defineEmits<{ reload: [] }>()
const { t } = useI18n()
</script>

<template>
  <div
    class="flex h-full flex-col items-center justify-center gap-3 p-6 text-center"
    role="alert"
  >
    <Icon
      name="lucide:puzzle"
      class="h-8 w-8 text-muted-foreground"
      :aria-hidden="true"
    />
    <p class="max-w-md text-sm text-muted-foreground">{{ message }}</p>
    <UiButton variant="outline" @click="emit('reload')">
      {{ t('extensions.frame.reload') }}
    </UiButton>
  </div>
</template>
