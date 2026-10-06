import { toast } from 'vue-sonner'
import type { CopyField } from '@bindings/CopyField'
import type { CopyResult } from '@bindings/CopyResult'
import type { HistorySecret } from '@bindings/HistorySecret'
import type { CopyTextSource } from '~/composables/usePasswords'

/**
 * The copy buttons of the password manager (spec 034, FR-017): every copy goes through Rust, which
 * puts the value on the clipboard and clears it after the vault's delay, and ends in one toast. A
 * stored value is copied by its field (`copyField`, `copyHistory`), so it never passes the
 * webview; a value the window holds already (an editor field) is sent as text, with the field it
 * comes from when it may hold placeholders, which Rust then resolves (spec 036, FR-045).
 */
export function usePasswordsCopy() {
  const { t } = useI18n()
  const { errString } = useErrorString()
  const { copyFieldAsync, copyTextAsync, historyCopyAsync } = usePasswords()

  async function runAsync(copy: () => Promise<CopyResult>, label: string) {
    try {
      const result = await copy()
      toast.success(
        result.clearsInSeconds === null
          ? t('passwords.copiedKept', { field: label })
          : t('passwords.copied', {
              field: label,
              seconds: result.clearsInSeconds,
            }),
      )
    } catch (cause) {
      toast.error(errString(cause))
    }
  }

  return {
    copyField: (itemId: string, field: CopyField, label: string) =>
      runAsync(() => copyFieldAsync(itemId, field), label),
    copyText: (text: string, label: string, from?: CopyTextSource) =>
      runAsync(() => copyTextAsync(text, from), label),
    copyHistory: (snapshotId: string, field: HistorySecret, label: string) =>
      runAsync(() => historyCopyAsync(snapshotId, field), label),
  }
}
