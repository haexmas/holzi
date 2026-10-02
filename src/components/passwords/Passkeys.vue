<script setup lang="ts">
/**
 * The passkeys of an entry (spec 034, FR-004): relying party, user name, created and last used,
 * rename the nickname, delete with a confirmation. There is no button to create one and a key is
 * never shown: passkeys arrive through an import or a sync, and signing comes later with the bridge.
 */
import { toast } from 'vue-sonner'
import type { PasskeyView } from '@bindings/PasskeyView'

defineProps<{
  itemId: string
  passkeys: PasskeyView[]
}>()

const emit = defineEmits<{
  changed: []
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const { renamePasskeyAsync, deletePasskeyAsync } = usePasswords()

const editing = ref<string | null>(null)
const nickname = ref('')
const deleting = ref<PasskeyView | null>(null)

const ALGORITHMS: Record<number, string> = {
  [-7]: 'ES256',
  [-8]: 'EdDSA',
  [-257]: 'RS256',
}

function algorithmLabel(algorithm: number): string {
  return (
    ALGORITHMS[algorithm] ??
    t('passwords.passkeys.otherAlgorithm', { number: algorithm })
  )
}

function startRename(passkey: PasskeyView) {
  editing.value = passkey.id
  nickname.value = passkey.nickname ?? ''
}

async function saveRenameAsync(passkey: PasskeyView) {
  try {
    await renamePasskeyAsync(passkey.id, nickname.value.trim() || null)
    editing.value = null
    emit('changed')
  } catch (cause) {
    toast.error(errString(cause))
  }
}

async function confirmDeleteAsync() {
  const target = deleting.value
  if (!target) return
  deleting.value = null
  try {
    await deletePasskeyAsync(target.id)
    emit('changed')
  } catch (cause) {
    toast.error(errString(cause))
  }
}
</script>

<template>
  <SettingsGroup v-if="passkeys.length" :label="t('passwords.passkeys.title')">
    <SettingsRow
      v-for="passkey in passkeys"
      :key="passkey.id"
      :title="
        passkey.nickname || passkey.relyingPartyName || passkey.relyingPartyId
      "
      :description="
        [
          passkey.relyingPartyId,
          passkey.userName,
          algorithmLabel(passkey.algorithm),
        ]
          .filter(Boolean)
          .join(' · ')
      "
      icon="lucide:fingerprint"
    >
      <template v-if="editing === passkey.id">
        <div class="w-40">
          <UiInput
            v-model="nickname"
            :aria-label="t('passwords.passkeys.nickname')"
            :data-testid="`passwords-passkey-nickname-${passkey.id}`"
            @keydown.enter.prevent="saveRenameAsync(passkey)"
            @keydown.esc.prevent="editing = null"
          />
        </div>
        <UiButton size="sm" @click="saveRenameAsync(passkey)">{{
          t('passwords.save')
        }}</UiButton>
      </template>
      <template v-else>
        <UiButton
          variant="ghost"
          size="icon"
          :aria-label="t('passwords.passkeys.rename')"
          :tooltip="t('passwords.passkeys.rename')"
          :data-testid="`passwords-passkey-rename-${passkey.id}`"
          @click="startRename(passkey)"
        >
          <Icon name="lucide:pencil" class="size-4" />
        </UiButton>
        <UiButton
          variant="ghost"
          size="icon"
          :aria-label="t('passwords.passkeys.delete')"
          :tooltip="t('passwords.passkeys.delete')"
          :data-testid="`passwords-passkey-delete-${passkey.id}`"
          @click="deleting = passkey"
        >
          <Icon name="lucide:trash-2" class="size-4" />
        </UiButton>
      </template>
      <template #below>
        <p
          class="text-xs text-muted-foreground"
          :data-testid="`passwords-passkey-${passkey.id}`"
        >
          {{
            t('passwords.passkeys.created', { date: passkey.createdAt ?? '–' })
          }}
          <template v-if="passkey.lastUsedAt">
            ·
            {{ t('passwords.passkeys.lastUsed', { date: passkey.lastUsedAt }) }}
          </template>
        </p>
      </template>
    </SettingsRow>
  </SettingsGroup>

  <ShadcnAlertDialog
    :open="deleting !== null"
    @update:open="(open: boolean) => !open && (deleting = null)"
  >
    <ShadcnAlertDialogContent v-if="deleting">
      <ShadcnAlertDialogHeader>
        <ShadcnAlertDialogTitle>{{
          t('passwords.passkeys.deleteTitle')
        }}</ShadcnAlertDialogTitle>
        <ShadcnAlertDialogDescription>
          {{
            t('passwords.passkeys.deleteBody', {
              name: deleting.relyingPartyName || deleting.relyingPartyId,
            })
          }}
        </ShadcnAlertDialogDescription>
      </ShadcnAlertDialogHeader>
      <ShadcnAlertDialogFooter>
        <ShadcnAlertDialogCancel>{{
          t('passwords.cancel')
        }}</ShadcnAlertDialogCancel>
        <UiButton variant="destructive" @click="confirmDeleteAsync">{{
          t('passwords.passkeys.delete')
        }}</UiButton>
      </ShadcnAlertDialogFooter>
    </ShadcnAlertDialogContent>
  </ShadcnAlertDialog>
</template>
