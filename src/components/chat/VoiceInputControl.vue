<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useEventListener } from '@vueuse/core'

/**
 * Mic control for voice dictation (spec 008-voice-control-stt, T014).
 *
 * Push-to-talk on every platform, desktop included: audio is recorded only
 * while the mic button is held (pointer, or Space/Enter on the focused
 * button). Releasing over the button ends the recording and transcribes it;
 * releasing anywhere else, pressing Escape or losing window focus discards
 * it. There is no click-to-toggle mode and no separate cancel button.
 *
 * US2 (the "stop"/"halt"/"abbrechen" interrupt fast path cancelling an
 * in-progress assistant turn) and US3 (choosing an external transcription
 * provider) are not implemented yet — `interrupt` on the result is only
 * used here to avoid writing/sending the bare interrupt word as a chat
 * message; nothing actually cancels the assistant turn (T019/T020), and
 * there is no persistent "external source" indicator yet (T027).
 */

type RecordingState = 'idle' | 'recording' | 'transcribing' | 'error'
type Release = 'send' | 'discard'

type TranscriptionResult = {
  text: string
  interrupt: 'stop' | 'halt' | 'abbrechen' | null
}

const AUTO_SEND_PREF_KEY = 'voice.auto_send'

/**
 * A hold shorter than this is a stray tap, not dictation: it is discarded
 * (a sub-300 ms clip only makes Whisper hallucinate) and the hold hint shows.
 */
const MIN_HOLD_MS = 300

const emit = defineEmits<{
  transcript: [text: string, autoSend: boolean]
}>()

const { t } = useI18n()
const { getPrefAsync, setPrefAsync } = usePreferences()

const state = ref<RecordingState>('idle')
const autoSend = ref(true)
const errorMessage = ref<string | null>(null)
const noSpeechDetected = ref(false)
const tooShort = ref(false)
/** The pointer is held outside the mic button, so releasing now discards. */
const releaseDiscards = ref(false)

let unlistenCapped: (() => void) | null = null
let disposed = false
let startPending = false
/** A press is down and its release has not been handled yet. */
let holding = false
let holdStartedAt = 0
/** The pressed button while the hold is pointer-driven; null for a key hold. */
let holdTarget: HTMLElement | null = null
let holdPointerId: number | null = null
/** A release that arrived while `start_voice_recording` was still in flight. */
let earlyRelease: Release | null = null

const micIcon = computed(() => {
  if (state.value === 'transcribing') return 'lucide:loader-2'
  return state.value === 'recording' && releaseDiscards.value
    ? 'lucide:x'
    : 'lucide:mic'
})
const micLabel = computed(() =>
  state.value === 'recording'
    ? t('voiceControl.mic.release')
    : t('voiceControl.mic.hold'),
)

onMounted(async () => {
  try {
    const stored = await getPrefAsync({ kind: 'vault' }, AUTO_SEND_PREF_KEY)
    // No stored value yet -> the documented default (FR-005: on).
    if (!disposed) autoSend.value = stored === null ? true : stored === 'true'
  } catch {
    // A failed preference read must not block dictation — keep the default.
  }
  const unlisten = await listen<TranscriptionResult | null>(
    'voice-recording-capped',
    ({ payload }) => {
      if (disposed || state.value !== 'recording') return
      endHold()
      void finishRecording(payload)
    },
  )
  if (disposed) unlisten()
  else unlistenCapped = unlisten
})

onBeforeUnmount(() => {
  disposed = true
  endHold()
  unlistenCapped?.()
  unlistenCapped = null
  if (startPending || state.value === 'recording') {
    void invoke('cancel_voice_recording').catch(() => {
      // The component is already going away; cancellation is best-effort.
    })
  }
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

async function startRecording(
  target: HTMLElement | null,
  pointerId: number | null,
) {
  if (
    disposed ||
    startPending ||
    (state.value !== 'idle' && state.value !== 'error')
  )
    return
  holding = true
  holdStartedAt = performance.now()
  holdTarget = target
  holdPointerId = pointerId
  earlyRelease = null
  releaseDiscards.value = false
  errorMessage.value = null
  noSpeechDetected.value = false
  tooShort.value = false
  startPending = true
  try {
    await invoke('start_voice_recording')
    if (disposed) {
      await invoke('cancel_voice_recording').catch(() => {
        // onBeforeUnmount also cancels; this covers a late start completion.
      })
      return
    }
    state.value = 'recording'
  } catch (e) {
    if (!disposed) {
      state.value = 'error'
      errorMessage.value = describeError(e)
    }
  } finally {
    startPending = false
  }
  // Released while the start was still in flight: settle it now.
  if (!holding && state.value === 'recording') {
    await settle(earlyRelease ?? 'send')
  }
}

async function finishRecording(result?: TranscriptionResult | null) {
  if (disposed) return
  state.value = 'transcribing'
  try {
    const transcription =
      result === undefined
        ? await invoke<TranscriptionResult>('stop_voice_recording')
        : result
    if (disposed) return
    if (transcription === null) {
      state.value = 'error'
      errorMessage.value = t('voiceControl.error.generic')
      return
    }
    state.value = 'idle'
    if (transcription.interrupt) return
    if (!transcription.text.trim()) {
      noSpeechDetected.value = true
      return
    }
    emit('transcript', transcription.text, autoSend.value)
  } catch (e) {
    if (disposed) return
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

function endHold() {
  holding = false
  holdTarget = null
  holdPointerId = null
  releaseDiscards.value = false
}

/** Ends the current hold. A no-op when no press is waiting for its release. */
function release(kind: Release) {
  if (!holding) return
  endHold()
  if (startPending) {
    earlyRelease = kind
    return
  }
  if (state.value === 'recording') void settle(kind)
}

async function settle(kind: Release) {
  const tap = performance.now() - holdStartedAt < MIN_HOLD_MS
  if (kind === 'send' && !tap) {
    await finishRecording()
    return
  }
  tooShort.value = kind === 'send'
  await cancelRecording()
}

function pointerOverTarget(e: PointerEvent): boolean {
  const rect = holdTarget?.getBoundingClientRect()
  return (
    !!rect &&
    e.clientX >= rect.left &&
    e.clientX <= rect.right &&
    e.clientY >= rect.top &&
    e.clientY <= rect.bottom
  )
}

function onPointerDown(e: PointerEvent) {
  // Primary button, touch or pen only: a right-click must not record.
  if (e.button !== 0) return
  const target = e.currentTarget as HTMLElement
  // Keeps pointer events flowing to us when the pointer leaves the button
  // (or the window) mid-hold, so the release is never lost.
  target.setPointerCapture(e.pointerId)
  void startRecording(target, e.pointerId)
}

function isHoldKey(e: KeyboardEvent) {
  return e.key === ' ' || e.key === 'Enter'
}

function onKeyDown(e: KeyboardEvent) {
  if (!isHoldKey(e)) return
  e.preventDefault()
  if (!e.repeat) void startRecording(null, null)
}

function onKeyUp(e: KeyboardEvent) {
  if (!isHoldKey(e)) return
  e.preventDefault()
  if (!holdTarget) release('send')
}

// A key hold ends when the button loses focus: its keyup would go elsewhere.
function onBlur() {
  if (!holdTarget) release('discard')
}

useEventListener(window, 'pointermove', (e) => {
  if (holdTarget && e.pointerId === holdPointerId) {
    releaseDiscards.value = !pointerOverTarget(e)
  }
})
useEventListener(window, 'pointerup', (e) => {
  if (holdTarget && e.pointerId === holdPointerId) {
    release(pointerOverTarget(e) ? 'send' : 'discard')
  }
})
useEventListener(window, 'pointercancel', (e) => {
  if (holdTarget && e.pointerId === holdPointerId) release('discard')
})
useEventListener(window, 'keydown', (e) => {
  if (e.key === 'Escape') release('discard')
})
// A hold cannot survive the window losing focus: its release would go
// undelivered and the recording would run until the length cap.
useEventListener(window, 'blur', () => release('discard'))
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
      class="touch-none select-none"
      :variant="state === 'recording' ? 'destructive' : 'secondary'"
      :disabled="state === 'transcribing'"
      :aria-pressed="state === 'recording'"
      :aria-label="micLabel"
      :title="micLabel"
      @pointerdown="onPointerDown"
      @keydown="onKeyDown"
      @keyup="onKeyUp"
      @blur="onBlur"
      @contextmenu.prevent
    >
      <Icon
        :name="micIcon"
        class="h-3.5 w-3.5"
        :class="{
          'animate-spin': state === 'transcribing',
          'animate-pulse text-destructive-foreground':
            state === 'recording' && !releaseDiscards,
        }"
      />
    </UiButton>
    <span
      v-if="noSpeechDetected"
      class="text-xs text-muted-foreground"
      role="status"
    >
      {{ t('voiceControl.noSpeechDetected') }}
    </span>
    <span v-if="tooShort" class="text-xs text-muted-foreground" role="status">
      {{ t('voiceControl.mic.hold') }}
    </span>
    <span v-if="errorMessage" class="text-xs text-destructive" role="alert">
      {{ errorMessage }}
    </span>
  </div>
</template>
