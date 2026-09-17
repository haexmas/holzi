<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/**
 * Mic control for voice dictation (spec 008-voice-control-stt, T014).
 *
 * US2 (the "stop"/"halt"/"abbrechen" interrupt fast path cancelling an
 * in-progress assistant turn) and US3 (choosing an external transcription
 * provider) are not implemented yet — `interrupt` on the result is only
 * used here to avoid writing/sending the bare interrupt word as a chat
 * message; nothing actually cancels the assistant turn (T019/T020), and
 * there is no persistent "external source" indicator yet (T027).
 */

type RecordingState = 'idle' | 'recording' | 'transcribing' | 'error'

type TranscriptionResult = {
  text: string
  interrupt: 'stop' | 'halt' | 'abbrechen' | null
}

const AUTO_SEND_PREF_KEY = 'voice.auto_send'

const emit = defineEmits<{
  transcript: [text: string, autoSend: boolean]
}>()

const { t } = useI18n()
const { getPrefAsync, setPrefAsync } = usePreferences()

const state = ref<RecordingState>('idle')
const autoSend = ref(true)
const errorMessage = ref<string | null>(null)
const noSpeechDetected = ref(false)

let unlistenCapped: (() => void) | null = null

onMounted(async () => {
  try {
    const stored = await getPrefAsync({ kind: 'vault' }, AUTO_SEND_PREF_KEY)
    // No stored value yet -> the documented default (FR-005: on).
    autoSend.value = stored === null ? true : stored === 'true'
  } catch {
    // A failed preference read must not block dictation — keep the default.
  }
  unlistenCapped = await listen('voice-recording-capped', () => {
    // Race note (contracts/tauri-commands.md): this event can arrive
    // before a manual `stop_voice_recording` call this component is
    // already awaiting. `finishRecording` is idempotent against that —
    // the backend coalesces both into one stop, so a second call here
    // just re-awaits the same result.
    if (state.value === 'recording') finishRecording()
  })
})

onBeforeUnmount(() => {
  unlistenCapped?.()
})

async function toggleAutoSend() {
  autoSend.value = !autoSend.value
  try {
    await setPrefAsync(
      { kind: 'vault' },
      AUTO_SEND_PREF_KEY,
      String(autoSend.value),
    )
  } catch {
    // Best-effort: the toggle still reflects the user's choice for this
    // session even if persisting it failed.
  }
}

function structuredVoiceError(e: unknown): { kind?: unknown } | null {
  if (e && typeof e === 'object' && 'kind' in e) {
    return e as { kind?: unknown }
  }
  if (typeof e !== 'string') return null
  try {
    const parsed: unknown = JSON.parse(e)
    if (parsed && typeof parsed === 'object' && 'kind' in parsed) {
      return parsed as { kind?: unknown }
    }
  } catch {
    // Native bridge may return a plain string; falls through to the
    // generic message below.
  }
  return null
}

function describeError(e: unknown): string {
  switch (structuredVoiceError(e)?.kind) {
    case 'DeviceUnavailable':
      return t('voiceControl.error.deviceUnavailable')
    case 'AlreadyRecording':
      return t('voiceControl.error.alreadyRecording')
    case 'TranscriptionFailed':
      return t('voiceControl.error.transcriptionFailed')
    default:
      return t('voiceControl.error.generic')
  }
}

async function startRecording() {
  if (state.value !== 'idle') return
  errorMessage.value = null
  noSpeechDetected.value = false
  try {
    await invoke('start_voice_recording')
    state.value = 'recording'
  } catch (e) {
    state.value = 'error'
    errorMessage.value = describeError(e)
  }
}

async function finishRecording() {
  state.value = 'transcribing'
  try {
    const result = await invoke<TranscriptionResult>('stop_voice_recording')
    state.value = 'idle'
    if (result.interrupt) return
    if (!result.text.trim()) {
      noSpeechDetected.value = true
      return
    }
    emit('transcript', result.text, autoSend.value)
  } catch (e) {
    state.value = 'error'
    errorMessage.value = describeError(e)
  }
}

async function cancelRecording() {
  try {
    await invoke('cancel_voice_recording')
  } finally {
    state.value = 'idle'
  }
}

function onMicClick() {
  if (state.value === 'idle' || state.value === 'error') startRecording()
  else if (state.value === 'recording') finishRecording()
}
</script>

<template>
  <div class="flex shrink-0 items-center gap-1.5">
    <button
      type="button"
      class="flex h-6 w-8 shrink-0 items-center justify-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-muted/60 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
      :class="{ 'text-muted-foreground/50': autoSend }"
      :aria-pressed="autoSend"
      :aria-label="
        autoSend
          ? t('voiceControl.autoSend.onLabel')
          : t('voiceControl.autoSend.offLabel')
      "
      :title="
        autoSend
          ? t('voiceControl.autoSend.onLabel')
          : t('voiceControl.autoSend.offLabel')
      "
      @click="toggleAutoSend"
    >
      <Icon
        :name="autoSend ? 'lucide:zap' : 'lucide:zap-off'"
        class="h-3.5 w-3.5"
      />
    </button>
    <UiButton
      type="button"
      size="icon-sm"
      :variant="state === 'recording' ? 'destructive' : 'secondary'"
      :disabled="state === 'transcribing'"
      :aria-label="
        state === 'recording'
          ? t('voiceControl.mic.stop')
          : t('voiceControl.mic.start')
      "
      :title="
        state === 'recording'
          ? t('voiceControl.mic.stop')
          : t('voiceControl.mic.start')
      "
      @click="onMicClick"
    >
      <Icon
        :name="state === 'transcribing' ? 'lucide:loader-2' : 'lucide:mic'"
        class="h-3.5 w-3.5"
        :class="{
          'animate-spin': state === 'transcribing',
          'animate-pulse text-destructive-foreground': state === 'recording',
        }"
      />
    </UiButton>
    <button
      v-if="state === 'recording'"
      type="button"
      class="text-xs text-muted-foreground underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
      @click="cancelRecording"
    >
      {{ t('voiceControl.mic.cancel') }}
    </button>
    <span
      v-if="noSpeechDetected"
      class="text-xs text-muted-foreground"
      role="status"
    >
      {{ t('voiceControl.noSpeechDetected') }}
    </span>
    <span v-if="errorMessage" class="text-xs text-destructive" role="alert">
      {{ errorMessage }}
    </span>
  </div>
</template>
