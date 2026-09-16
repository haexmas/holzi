import { hfErrorKey } from '~/composables/useHuggingFace'

const HF_ERROR_KINDS = new Set([
  'InvalidInput',
  'Network',
  'Timeout',
  'HttpStatus',
  'RateLimited',
  'UnsupportedFormat',
  'TokenizerRequired',
  'HardwareConfirmationRequired',
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
      if (kind === 'InvalidIdempotencyKey')
        return t('errors.invalidIdempotencyKey')
      if (kind === 'IdempotencyKeyConflict')
        return t('errors.idempotencyKeyConflict')
      if (typeof kind === 'string' && HF_ERROR_KINDS.has(kind))
        return t(hfErrorKey(e))
      return JSON.stringify(e)
    }
    return String(e)
  }

  return { errString }
}
