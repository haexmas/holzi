<script setup lang="ts">
/**
 * The history of an entry (spec 034, US4, FR-017, FR-018): every state newest first with its time
 * and the names of the fields that changed against the state before; opening one shows it masked,
 * a secret only on "show", and "Restore" makes it the current state while the state it replaces
 * stays in the list. Restoring checks that the entry has not changed meanwhile.
 */
import { toast } from 'vue-sonner'
import type { SnapshotHeader } from '@bindings/SnapshotHeader'
import type { SnapshotView } from '@bindings/SnapshotView'
import { displayTitle } from '~/lib/passwords/format'

const props = defineProps<{
  itemId?: string
}>()

const { t, d } = useI18n()
const { errString } = useErrorString()
const router = useTabRouter()
const store = usePasswordsStore()
const {
  historyListAsync,
  historyGetAsync,
  historyRevealAsync,
  historyRestoreAsync,
  getItemAsync,
} = usePasswords()

const id = computed(() => props.itemId ?? router.route.params.id ?? '')
const states = ref<SnapshotHeader[]>([])
const selected = ref<SnapshotView | null>(null)
const title = ref<string | null>(null)
const error = ref<string | null>(null)
const loading = ref(true)
const restoring = ref(false)

async function loadAsync() {
  try {
    const [list, detail] = await Promise.all([
      historyListAsync(id.value),
      getItemAsync(id.value),
    ])
    states.value = list
    title.value = displayTitle(detail.title)
    error.value = null
  } catch (cause) {
    error.value = errString(cause)
  } finally {
    loading.value = false
  }
}

async function selectAsync(snapshotId: string) {
  try {
    selected.value = await historyGetAsync(snapshotId)
  } catch (cause) {
    toast.error(errString(cause))
  }
}

function when(stamp: string | null): string {
  if (!stamp) return '–'
  const date = new Date(stamp)
  return Number.isNaN(date.getTime())
    ? stamp
    : d(date, { dateStyle: 'medium', timeStyle: 'medium' })
}

function changedLabel(names: string[]): string {
  return names.map((name) => t(`passwords.history.fields.${name}`)).join(', ')
}

async function restoreAsync() {
  const view = selected.value
  if (!view) return
  restoring.value = true
  try {
    // The token of the entry as it is now: a change meanwhile is a conflict, not an overwrite.
    const current = await getItemAsync(id.value)
    if (!current.updatedAt) throw new Error('missing update token')
    const outcome = await historyRestoreAsync(
      id.value,
      view.id,
      current.updatedAt,
    )
    await store.quietReloadAsync()
    if (outcome.skippedAttachments.length > 0) {
      toast.warning(
        t('passwords.history.skipped', {
          files: outcome.skippedAttachments.join(', '),
        }),
      )
    } else {
      toast.success(t('passwords.history.restored'))
    }
    router.replace(`/entry/${id.value}`)
  } catch (cause) {
    toast.error(errString(cause))
  } finally {
    restoring.value = false
  }
}

onMounted(loadAsync)
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-6 @md:px-6">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-4">
      <div class="flex items-center gap-2 pt-1">
        <UiButton
          variant="ghost"
          size="icon"
          class="-ml-2 shrink-0"
          :aria-label="t('passwords.back')"
          data-testid="passwords-history-back"
          @click="router.back()"
        >
          <Icon name="lucide:arrow-left" class="size-5" />
        </UiButton>
        <h1
          class="min-w-0 flex-1 truncate text-2xl font-bold"
          data-testid="passwords-history-title"
        >
          {{ t('passwords.history.title')
          }}<template v-if="title"> · {{ title }}</template>
        </h1>
      </div>

      <p v-if="error" class="text-sm text-destructive" role="alert">
        {{ error }}
      </p>
      <div
        v-else-if="loading"
        class="flex justify-center py-10 text-muted-foreground"
        role="status"
      >
        <Icon name="lucide:loader-circle" class="size-6 animate-spin" />
      </div>

      <template v-else>
        <SettingsGroup :label="t('passwords.history.states')">
          <SettingsRow
            v-for="state in states"
            :key="state.id"
            :title="when(state.modifiedAt)"
            :description="changedLabel(state.changedFields)"
            navigates
            :data-testid="`passwords-history-state-${state.id}`"
            @select="selectAsync(state.id)"
          />
        </SettingsGroup>

        <template v-if="selected">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="min-w-0 flex-1 text-lg font-semibold">
              {{ when(selected.modifiedAt) }}
            </h2>
            <UiButton
              :loading="restoring"
              data-testid="passwords-history-restore"
              @click="restoreAsync"
            >
              <Icon name="lucide:rotate-ccw" class="size-4" />
              {{ t('passwords.history.restore') }}
            </UiButton>
          </div>
          <SettingsGroup>
            <SettingsRow
              :title="t('passwords.fields.title')"
              :description="selected.title ?? t('passwords.noValue')"
            />
            <SettingsRow
              :title="t('passwords.fields.username')"
              :description="selected.username ?? t('passwords.noValue')"
            />
            <SettingsRow :title="t('passwords.fields.password')">
              <PasswordsMaskedValue
                :fetch="
                  async () =>
                    (
                      await historyRevealAsync(selected!.id, {
                        kind: 'password',
                      })
                    ).value
                "
                :identity="`${selected.id}:password`"
                kind="history-password"
                :present="selected.hasPassword"
                :label="t('passwords.fields.password')"
              />
            </SettingsRow>
            <SettingsRow
              :title="t('passwords.fields.url')"
              :description="selected.url ?? t('passwords.noValue')"
            />
            <SettingsRow
              v-if="selected.hasOtpSecret"
              :title="t('passwords.fields.totp')"
            >
              <PasswordsMaskedValue
                :fetch="
                  async () =>
                    (
                      await historyRevealAsync(selected!.id, {
                        kind: 'otpSecret',
                      })
                    ).value
                "
                :identity="`${selected.id}:otp`"
                kind="history-otp"
                :present="true"
                :label="t('passwords.fields.totp')"
              />
            </SettingsRow>
            <SettingsRow
              v-if="selected.expiresAt"
              :title="t('passwords.fields.expires')"
              :description="selected.expiresAt"
            />
            <SettingsRow
              v-if="selected.tags.length"
              :title="t('passwords.fields.tags')"
              :description="selected.tags.join(', ')"
            />
            <SettingsRow
              v-for="(field, index) in selected.keyValues"
              :key="index"
              :title="field.key ?? ''"
            >
              <PasswordsMaskedValue
                :fetch="
                  async () =>
                    (
                      await historyRevealAsync(selected!.id, {
                        kind: 'keyValue',
                        key: field.key ?? '',
                      })
                    ).value
                "
                :identity="`${selected.id}:kv:${field.key}`"
                kind="history-keyvalue"
                :present="field.hasValue"
                :label="field.key ?? ''"
              />
            </SettingsRow>
            <SettingsRow
              v-for="attachment in selected.attachments"
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
            v-if="selected.note"
            class="whitespace-pre-wrap rounded-xl bg-muted px-4 py-3 text-sm"
          >
            {{ selected.note }}
          </p>
        </template>
      </template>
    </div>
  </div>
</template>
