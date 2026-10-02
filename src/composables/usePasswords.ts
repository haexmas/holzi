import { invoke } from '@tauri-apps/api/core'
import type { CopyField } from '@bindings/CopyField'
import type { CopyResult } from '@bindings/CopyResult'
import type { CreateGroupResult } from '@bindings/CreateGroupResult'
import type { CreateItemResult } from '@bindings/CreateItemResult'
import type { GroupPatch } from '@bindings/GroupPatch'
import type { ItemDetail } from '@bindings/ItemDetail'
import type { ItemInput } from '@bindings/ItemInput'
import type { ItemPatch } from '@bindings/ItemPatch'
import type { Preset } from '@bindings/Preset'
import type { PresetInput } from '@bindings/PresetInput'
import type { PresetSaveResult } from '@bindings/PresetSaveResult'
import type { MoveResult } from '@bindings/MoveResult'
import type { Overview } from '@bindings/Overview'
import type { RevealedSecret } from '@bindings/RevealedSecret'
import type { SecretField } from '@bindings/SecretField'
import type { SetTagsResult } from '@bindings/SetTagsResult'
import type { Target } from '@bindings/Target'
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

  const createGroupAsync = (args: {
    name: string
    description?: string
    icon?: string
    color?: string
    parentId?: string
  }) => invoke<CreateGroupResult>('passwords_create_group', { args })

  const updateGroupAsync = (groupId: string, patch: GroupPatch) =>
    invoke<null>('passwords_update_group', { args: { groupId, patch } })

  const reorderGroupsAsync = (parentId: string | null, orderedIds: string[]) =>
    invoke<null>('passwords_reorder_groups', {
      args: { parentId, orderedIds },
    })

  const moveAsync = (targets: Target[], toGroupId: string | null) =>
    invoke<MoveResult>('passwords_move', { args: { targets, toGroupId } })

  const setTagsAsync = (itemIds: string[], add: string[], remove: string[]) =>
    invoke<SetTagsResult>('passwords_set_tags', {
      args: { itemIds, add, remove },
    })

  const renameTagAsync = (tagId: string, name: string) =>
    invoke<null>('passwords_rename_tag', { args: { tagId, name } })

  const setTagColorAsync = (tagId: string, color: string | null) =>
    invoke<null>('passwords_set_tag_color', { args: { tagId, color } })

  const deleteTagAsync = (tagId: string) =>
    invoke<null>('passwords_delete_tag', { args: { tagId } })

  const presetListAsync = () => invoke<Preset[]>('passwords_preset_list')

  const presetSaveAsync = (preset: PresetInput) =>
    invoke<PresetSaveResult>('passwords_preset_save', { args: { preset } })

  const presetDeleteAsync = (presetId: string) =>
    invoke<null>('passwords_preset_delete', { args: { presetId } })

  return {
    presetListAsync,
    presetSaveAsync,
    presetDeleteAsync,
    createGroupAsync,
    updateGroupAsync,
    reorderGroupsAsync,
    moveAsync,
    setTagsAsync,
    renameTagAsync,
    setTagColorAsync,
    deleteTagAsync,
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
