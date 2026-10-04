<script setup lang="ts">
/**
 * The tab Details of an entry in view mode (spec 036, FR-001, FR-005; fields from spec 034 FR-002):
 * username, password (masked until asked), address, expiry, the live TOTP block, the note and the
 * tags. Empty fields are not shown. Copying goes through the parent (`copy`), which also shows the
 * toast; no value passes through here except what the masked value asks for.
 */
import type { CopyField } from '@bindings/CopyField'
import type { ItemDetail } from '@bindings/ItemDetail'

defineProps<{
  itemId: string
  detail: ItemDetail
}>()
const emit = defineEmits<{
  copy: [field: CopyField, label: string]
  edit: []
  removeOtp: []
}>()

const { t } = useI18n()
const { revealAsync } = usePasswords()
</script>

<template>
  <div class="flex flex-col gap-4" data-testid="entry-view-details">
    <SettingsGroup>
      <SettingsRow
        v-if="detail.username"
        :title="t('passwords.fields.username')"
      >
        <PasswordsReferenceValue
          v-if="detail.references.username.length"
          :marks="detail.references.username"
          :text="detail.username"
          kind="username"
        />
        <span
          v-else
          class="min-w-0 truncate"
          data-testid="passwords-value-username"
          >{{ detail.username }}</span
        >
        <UiButton
          variant="ghost"
          size="icon"
          :aria-label="
            t('passwords.copy', { field: t('passwords.fields.username') })
          "
          data-testid="passwords-copy-username"
          @click="
            emit('copy', { kind: 'username' }, t('passwords.fields.username'))
          "
        >
          <Icon name="lucide:copy" class="size-4" />
        </UiButton>
      </SettingsRow>
      <SettingsRow :title="t('passwords.fields.password')">
        <template v-if="detail.references.password.length" #below>
          <PasswordsReferenceValue
            :marks="detail.references.password"
            :text="null"
            kind="password"
          />
        </template>
        <PasswordsMaskedValue
          :fetch="
            async () => (await revealAsync(itemId, { kind: 'password' })).value
          "
          :identity="`${itemId}:password`"
          kind="password"
          :present="detail.hasPassword"
          :label="t('passwords.fields.password')"
        />
        <UiButton
          v-if="detail.hasPassword"
          variant="ghost"
          size="icon"
          :aria-label="
            t('passwords.copy', { field: t('passwords.fields.password') })
          "
          data-testid="passwords-copy-password"
          @click="
            emit('copy', { kind: 'password' }, t('passwords.fields.password'))
          "
        >
          <Icon name="lucide:copy" class="size-4" />
        </UiButton>
      </SettingsRow>
      <SettingsRow v-if="detail.url" :title="t('passwords.fields.url')">
        <PasswordsReferenceValue
          v-if="detail.references.url.length"
          :marks="detail.references.url"
          :text="detail.url"
          kind="url"
        />
        <span
          v-else
          class="min-w-0 truncate"
          data-testid="passwords-value-url"
          >{{ detail.url }}</span
        >
      </SettingsRow>
      <SettingsRow
        v-if="detail.expiresAt"
        :title="t('passwords.fields.expires')"
      >
        <span>{{ detail.expiresAt }}</span>
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup
      v-if="detail.otpState !== 'none'"
      :label="t('passwords.fields.totp')"
    >
      <li class="px-4 py-3" data-no-swipe>
        <PasswordsTotpCode
          :item-id="itemId"
          :state="detail.otpState"
          @copy="emit('copy', { kind: 'totp' }, t('passwords.fields.totp'))"
          @replace="emit('edit')"
          @remove="emit('removeOtp')"
        />
      </li>
    </SettingsGroup>

    <SettingsGroup v-if="detail.note" :label="t('passwords.fields.note')">
      <li
        class="px-4 py-3 text-sm whitespace-pre-wrap"
        data-testid="passwords-value-note"
      >
        <PasswordsReferenceValue
          v-if="detail.references.note.length"
          :marks="detail.references.note"
          :text="detail.note"
          kind="note"
        />
        <template v-else>{{ detail.note }}</template>
      </li>
    </SettingsGroup>

    <div
      v-if="detail.tags.length"
      class="flex flex-wrap gap-1.5"
      data-testid="passwords-entry-tags"
    >
      <ShadcnBadge v-for="tag in detail.tags" :key="tag.id" variant="secondary">
        {{ tag.name }}
      </ShadcnBadge>
    </div>
  </div>
</template>
