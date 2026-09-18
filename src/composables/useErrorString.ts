/** contracts/tauri-commands.md (spec 009-autonomous-delegate-mode): the
 * `send_message` `InvalidInput` reason prefix for an autonomy mode the
 * connected delegate's installed CLI does not support (FR-013/SC-006).
 * Checked before the generic `InvalidInput` fallback below — other
 * `InvalidInput` errors keep their existing mapping. */
const AUTONOMY_UNAVAILABLE_PREFIX =
  'adapter start: backend unavailable: autonomy mode unavailable:'

const HF_ERROR_KINDS = new Set([
  'InvalidInput',
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
      if (
        kind === 'InvalidInput' &&
        typeof reason === 'string' &&
        reason.startsWith(AUTONOMY_UNAVAILABLE_PREFIX)
      )
        return t('errors.autonomyUnavailable')
      if (typeof kind === 'string' && HF_ERROR_KINDS.has(kind))
        return t(hfErrorKey(e))
      return JSON.stringify(e)
    }
    return String(e)
  }

  return { errString }
}
