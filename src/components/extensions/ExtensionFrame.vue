<script setup lang="ts">
/**
 * The content of an extension tab (spec 017, T046): a sandboxed frame with scripts only — no
 * `allow-same-origin`, popups, top navigation or forms (research R12) — served by the `holzi-ext`
 * scheme with the start token of its frame session.
 */
import { ref } from 'vue'
import { useExtensionFrame } from '~/composables/useExtensionFrame'

const props = defineProps<{ extensionId: string }>()
const { t } = useI18n()

const iframe = ref<HTMLIFrameElement | null>(null)
const { state, error, src, dialog, answerDialog, onLoad, reloadAsync } =
  useExtensionFrame(iframe, props.extensionId)
</script>

<template>
  <div class="relative h-full min-h-0">
    <iframe
      v-if="src"
      ref="iframe"
      :src="src"
      sandbox="allow-scripts"
      referrerpolicy="no-referrer"
      class="h-full w-full border-0"
      :class="{ invisible: state !== 'ready' }"
      :title="t('extensions.frame.title')"
      @load="onLoad"
    />
    <div
      v-if="state === 'loading'"
      class="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground"
    >
      {{ t('extensions.frame.loading') }}
    </div>
    <ExtensionsFrameError
      v-if="state === 'error'"
      class="absolute inset-0"
      :message="error ?? t('extensions.frame.failed')"
      @reload="reloadAsync"
    />
    <ExtensionsFrameDialog
      v-if="dialog"
      class="absolute inset-0"
      :dialog="dialog"
      @answer="answerDialog"
    />
  </div>
</template>
