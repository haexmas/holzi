<script setup lang="ts">
/**
 * Tells the user, once, that this vault file is the copy of a main device and that this device is
 * now a main device too (spec 024, user story 7, FR-044). Shown above the desktop until the user
 * dismisses it; the backend remembers the dismissal. Nothing shows for a device that did not
 * enroll as a copy, or while the status cannot be read.
 */
const { t } = useI18n()
const { syncStatusAsync, copyNoticeDismissAsync } = useSync()

const visible = ref(false)

onMounted(async () => {
  try {
    visible.value = (await syncStatusAsync()).copyEnrolledAsMain
  } catch (error) {
    console.error('[sync] reading the copy notice failed', error)
  }
})

async function onDismiss() {
  visible.value = false
  try {
    await copyNoticeDismissAsync()
  } catch (error) {
    console.error('[sync] dismissing the copy notice failed', error)
  }
}
</script>

<template>
  <div
    v-if="visible"
    class="flex items-center gap-3 bg-muted px-4 py-2 text-sm"
    role="status"
    data-testid="copy-notice"
  >
    <p class="min-w-0 flex-1">{{ t('settings.admission.copyNotice') }}</p>
    <UiButton
      variant="outline"
      data-testid="copy-notice-dismiss"
      @click="onDismiss"
    >
      {{ t('settings.admission.dismiss') }}
    </UiButton>
  </div>
</template>
