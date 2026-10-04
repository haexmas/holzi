<script setup lang="ts">
/**
 * One state of an entry, read only (spec 036, US2, FR-007; spec 034 FR-017): its time, the fields
 * with the secrets masked until asked (and hidden again when the state or the tab changes: the
 * parent changes `resetKey`), and "Restore". The restore button only asks; the parent confirms and
 * writes.
 */
import type { SnapshotView } from '@bindings/SnapshotView'

defineProps<{
  snapshot: SnapshotView
  restoring: boolean
  /** Changes when the secrets must hide again (the Verlauf tab was left). */
  resetKey: number
}>()
const emit = defineEmits<{ restore: [] }>()

const { t, d } = useI18n()
const { historyRevealAsync } = usePasswords()

function when(stamp: string | null): string {
  if (!stamp) return '–'
  const date = new Date(stamp)
  return Number.isNaN(date.getTime())
    ? stamp
    : d(date, { dateStyle: 'medium', timeStyle: 'medium' })
}
</script>

<template>
  <div
    class="flex min-w-0 flex-col gap-4"
    data-testid="passwords-history-snapshot"
  >
    <div class="flex flex-wrap items-center gap-2">
      <h2 class="min-w-0 flex-1 text-lg font-semibold">
        {{
          t('passwords.history.savedAt', { when: when(snapshot.modifiedAt) })
        }}
      </h2>
      <UiButton
        :loading="restoring"
        data-testid="passwords-history-restore"
        @click="emit('restore')"
      >
        <Icon name="lucide:rotate-ccw" class="size-4" />
        {{ t('passwords.history.restore') }}
      </UiButton>
    </div>
    <SettingsGroup>
      <SettingsRow
        :title="t('passwords.fields.title')"
        :description="snapshot.title ?? t('passwords.noValue')"
      />
      <SettingsRow
        :title="t('passwords.fields.username')"
        :description="snapshot.username ?? t('passwords.noValue')"
      />
      <SettingsRow :title="t('passwords.fields.password')">
        <PasswordsMaskedValue
          :fetch="
            async () =>
              (await historyRevealAsync(snapshot.id, { kind: 'password' }))
                .value
          "
          :identity="`${snapshot.id}:password:${resetKey}`"
          kind="history-password"
          :present="snapshot.hasPassword"
          :label="t('passwords.fields.password')"
        />
      </SettingsRow>
      <SettingsRow
        :title="t('passwords.fields.url')"
        :description="snapshot.url ?? t('passwords.noValue')"
      />
      <SettingsRow
        v-if="snapshot.hasOtpSecret"
        :title="t('passwords.fields.totp')"
      >
        <PasswordsMaskedValue
          :fetch="
            async () =>
              (await historyRevealAsync(snapshot.id, { kind: 'otpSecret' }))
                .value
          "
          :identity="`${snapshot.id}:otp:${resetKey}`"
          kind="history-otp"
          :present="true"
          :label="t('passwords.fields.totp')"
        />
      </SettingsRow>
      <SettingsRow
        v-if="snapshot.expiresAt"
        :title="t('passwords.fields.expires')"
        :description="snapshot.expiresAt"
      />
      <SettingsRow
        v-if="snapshot.tags.length"
        :title="t('passwords.fields.tags')"
        :description="snapshot.tags.join(', ')"
      />
      <SettingsRow
        v-for="(field, index) in snapshot.keyValues"
        :key="index"
        :title="field.key ?? ''"
      >
        <PasswordsMaskedValue
          :fetch="
            async () =>
              (
                await historyRevealAsync(snapshot.id, {
                  kind: 'keyValue',
                  key: field.key ?? '',
                })
              ).value
          "
          :identity="`${snapshot.id}:kv:${field.key}:${resetKey}`"
          kind="history-keyvalue"
          :present="field.hasValue"
          :label="field.key ?? ''"
        />
      </SettingsRow>
      <SettingsRow
        v-for="attachment in snapshot.attachments"
        :key="attachment.binaryHash + attachment.fileName"
        :title="attachment.fileName"
        :description="
          attachment.available
            ? undefined
            : t('passwords.history.attachmentGone')
        "
        icon="lucide:paperclip"
      />
    </SettingsGroup>
    <p
      v-if="snapshot.note"
      class="rounded-xl bg-muted px-4 py-3 text-sm whitespace-pre-wrap"
    >
      {{ snapshot.note }}
    </p>
  </div>
</template>
