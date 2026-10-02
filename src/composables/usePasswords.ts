import { invoke } from '@tauri-apps/api/core'
import type { CopyField } from '@bindings/CopyField'
import type { CopyResult } from '@bindings/CopyResult'
import type { CreateItemResult } from '@bindings/CreateItemResult'
import type { ItemDetail } from '@bindings/ItemDetail'
import type { ItemInput } from '@bindings/ItemInput'
import type { ItemPatch } from '@bindings/ItemPatch'
import type { Overview } from '@bindings/Overview'
import type { RevealedSecret } from '@bindings/RevealedSecret'
import type { SecretField } from '@bindings/SecretField'
import type { TotpCode } from '@bindings/TotpCode'
import type { UpdateItemResult } from '@bindings/UpdateItemResult'

/**
 * Thin `invoke` wrappers of the password manager commands (spec 034,
 * `contracts/tauri-commands.md`). Arguments travel in one `args` object; the backend decides who the
 * caller is, never the frontend. A secret comes back only from `revealAsync`; copying and the TOTP
 * code never hand it over.
 */
export function usePasswords() {
  const loadOverviewAsync = () => invoke<Overview>('passwords_load_overview')

  const getItemAsync = (itemId: string) =>
    invoke<ItemDetail>('passwords_get_item', { args: { itemId } })

  const revealAsync = (itemId: string, field: SecretField) =>
    invoke<RevealedSecret>('passwords_reveal', { args: { itemId, field } })

  const totpCodeAsync = (itemId: string) =>
    invoke<TotpCode>('passwords_totp_code', { args: { itemId } })

  const copyFieldAsync = (itemId: string, field: CopyField) =>
    invoke<CopyResult>('passwords_copy_field', { args: { itemId, field } })

  const createItemAsync = (input: ItemInput, groupId?: string) =>
    invoke<CreateItemResult>('passwords_create_item', {
      args: { input, groupId },
    })

  const updateItemAsync = (
    itemId: string,
    expectedUpdatedAt: string,
    patch: ItemPatch,
  ) =>
    invoke<UpdateItemResult>('passwords_update_item', {
      args: { itemId, expectedUpdatedAt, patch },
    })

  const renamePasskeyAsync = (passkeyId: string, nickname: string | null) =>
    invoke<null>('passwords_passkey_rename', { args: { passkeyId, nickname } })

  const deletePasskeyAsync = (passkeyId: string) =>
    invoke<null>('passwords_passkey_delete', { args: { passkeyId } })

  return {
    loadOverviewAsync,
    getItemAsync,
    revealAsync,
    totpCodeAsync,
    copyFieldAsync,
    createItemAsync,
    updateItemAsync,
    renamePasskeyAsync,
    deletePasskeyAsync,
  }
}
