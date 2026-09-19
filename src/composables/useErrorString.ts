/** contracts/tauri-commands.md (spec 009-autonomous-delegate-mode): the
 * `send_message` `InvalidInput` reason prefix for an autonomy mode the
 * connected delegate's installed CLI does not support (FR-013/SC-006).
 * Checked before the generic `InvalidInput` fallback below — other
 * `InvalidInput` errors keep their existing mapping. */
const AUTONOMY_UNAVAILABLE_PREFIX =
  'adapter start: backend unavailable: autonomy mode unavailable:'

// `InvalidInput` is deliberately excluded: unlike the kinds below, it's a
// generic catch-all bucket reused across the whole backend (providers,
// chat, voice, device commands — see src-tauri/src/providers/mod.rs
// map_adapter_error), not something only HF commands return, so it must
// fall through to the `reason`-based generic mapping below instead of
// this HF-flavored one.
const HF_ERROR_KINDS = new Set([
  'Network',
  'Timeout',
  'HttpStatus',
  'RateLimited',
  'UnsupportedFormat',
  'TokenizerRequired',
  'HardwareConfirmationRequired',
  'ModelDownload',
  'ModelRegistrationFailed',
  'ModelNotFound',
])

/**
 * Converts backend and JavaScript failures into displayable text. Shared by
 * the chat page (composer/thread errors) and `useModelsStore` (model
 * lifecycle errors) so the HF/idempotency-kind mapping has one source.
 */
export function useErrorString() {
  const { t } = useI18n()

  /** Maps a caught value to a localized or safely serialized error message. */
  function errString(e: unknown): string {
    if (typeof e === 'string') return e
    if (e && typeof e === 'object' && 'kind' in e) {
      const kind = (e as { kind: unknown }).kind
      const reason = (e as { reason?: unknown }).reason
      if (kind === 'InvalidIdempotencyKey')
        return t('errors.invalidIdempotencyKey')
      if (kind === 'IdempotencyKeyConflict')
        return t('errors.idempotencyKeyConflict')
      if (kind === 'InvalidInput' && typeof reason === 'string') {
        if (reason.startsWith(AUTONOMY_UNAVAILABLE_PREFIX))
          return t('errors.autonomyUnavailable')
        // Generic across providers/chat/voice/device commands (see
        // `HF_ERROR_KINDS`'s own comment) — a localized label plus the
        // backend's own reason, the same "translated frame + raw detail"
        // shape `HuggingFaceModelManagement.vue` builds by hand from
        // `hfErrorKey`/`hfErrorDetail`, so a real reason like "claude not
        // found on PATH" stays visible instead of being replaced outright
        // by a canned message.
        return reason.length > 0
          ? `${t('errors.invalidInput')}: ${reason}`
          : t('errors.invalidInput')
      }
      if (typeof kind === 'string' && HF_ERROR_KINDS.has(kind))
        return t(hfErrorKey(e))
      // HolziError variants serialize as `{ kind, ...fields }` with no
      // Display-rendered message on the wire (see src-tauri/src/error.rs) —
      // `reason` is the closest thing to human-readable text most variants
      // carry. Falls back to the raw shape only for the few fieldless
      // variants (e.g. WrongPassphrase) that have no text at all.
      if (typeof reason === 'string' && reason.length > 0) return reason
      return JSON.stringify(e)
    }
    if (e instanceof Error) return e.message
    return String(e)
  }

  return { errString }
}
