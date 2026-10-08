<script setup lang="ts">
/**
 * Video and audio from the media server (spec 044 FR-009, FR-012): the element's own controls;
 * the server answers ranges, so seeking does not load the file up to that point. A file the web
 * view cannot play reports `failed`, and the viewer shows the info view with "open with system
 * app" instead.
 */
defineProps<{ url: string; kind: 'video' | 'audio' }>()
const emit = defineEmits<{ failed: [] }>()
</script>

<template>
  <div class="flex h-full items-center justify-center p-2">
    <video
      v-if="kind === 'video'"
      :src="url"
      controls
      preload="metadata"
      class="max-h-full max-w-full"
      data-testid="files-viewer-video"
      @error="emit('failed')"
    />
    <audio
      v-else
      :src="url"
      controls
      preload="metadata"
      class="w-full max-w-md"
      data-testid="files-viewer-audio"
      @error="emit('failed')"
    />
  </div>
</template>
