<script setup lang="ts">
/**
 * The view of one entry (spec 034, US1, FR-002, FR-003, FR-005, FR-006): its fields with copy
 * buttons, the password and the custom fields masked until the user asks, the live TOTP block and
 * the passkeys. The detail the backend sends holds flags, never a secret; the tab title stays the
 * static place title, never an entry value (FR-040).
 */
import { toast } from 'vue-sonner'
import type { CopyField } from '@bindings/CopyField'
import type { ItemDetail } from '@bindings/ItemDetail'
import { displayTitle, isExpired, localDay } from '~/lib/passwords/format'

const props = defineProps<{
  itemId: string
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const router = useTabRouter()
const store = usePasswordsStore()
const { getItemAsync, copyFieldAsync, updateItemAsync, revealAsync } =
  usePasswords()

const detail = ref<ItemDetail | null>(null)
const deleteOpen = ref(false)
const notFound = ref(false)
const error = ref<string | null>(null)

async function loadAsync() {
  try {
    detail.value = await getItemAsync(props.itemId)
    notFound.value = false
    error.value = null
  } catch (cause) {
    if ((cause as { kind?: string }).kind === 'PasswordsNotFound') {
      detail.value = null
      notFound.value = true
    } else {
      error.value = errString(cause)
    }
  }
}

// A change from another window, an agent or a sync reloads the entry quietly.
const token = computed(() => store.headersById.get(props.itemId)?.updatedAt)
watch(token, () => {
  if (detail.value) void loadAsync()
})
onMounted(loadAsync)

const title = computed(() => displayTitle(detail.value?.title))
const expired = computed(() =>
  isExpired(detail.value?.expiresAt, localDay(new Date())),
)

async function copyAsync(field: CopyField, label: string) {
  try {
    const result = await copyFieldAsync(props.itemId, field)
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

function edit() {
  router.push(`/entry/${props.itemId}?edit`)
}

/** Removes an invalid TOTP secret without touching anything else of the entry. */
async function removeOtpAsync() {
  if (!detail.value?.updatedAt) return
  try {
    await updateItemAsync(props.itemId, detail.value.updatedAt, {
      otpSecret: null,
    })
    await store.quietReloadAsync()
    await loadAsync()
  } catch (cause) {
    error.value = errString(cause)
  }
}
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
          :tooltip="t('passwords.back')"
          data-testid="passwords-back"
          @click="router.back()"
        >
          <Icon name="lucide:arrow-left" class="size-5" />
        </UiButton>
        <h1
          v-if="detail"
          class="min-w-0 flex-1 truncate text-2xl font-bold"
          :class="title === null ? 'text-muted-foreground italic' : ''"
          data-testid="passwords-entry-title"
        >
          {{ title ?? t('passwords.untitled') }}
        </h1>
        <UiButton
          v-if="detail"
          variant="outline"
          class="ml-auto shrink-0"
          data-testid="passwords-edit"
          @click="edit"
        >
          <Icon name="lucide:pencil" class="size-4" />
          {{ t('passwords.edit') }}
        </UiButton>
        <UiButton
          v-if="detail"
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('passwords.history.open')"
          :tooltip="t('passwords.history.open')"
          data-testid="passwords-history"
          @click="router.push(`/entry/${itemId}/history`)"
        >
          <Icon name="lucide:history" class="size-4" />
        </UiButton>
        <UiButton
          v-if="detail"
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('passwords.delete.title')"
          :tooltip="t('passwords.delete.title')"
          data-testid="passwords-delete"
          @click="deleteOpen = true"
        >
          <Icon name="lucide:trash-2" class="size-4" />
        </UiButton>
      </div>

      <p
        v-if="notFound"
        class="py-10 text-center text-muted-foreground"
        role="alert"
      >
        {{ t('passwords.entryGone') }}
      </p>
      <p v-else-if="error" class="text-sm text-destructive" role="alert">
        {{ error }}
      </p>
      <div
        v-else-if="!detail"
        class="flex justify-center py-10 text-muted-foreground"
        role="status"
      >
        <Icon name="lucide:loader-circle" class="size-6 animate-spin" />
      </div>

      <template v-else>
        <ShadcnBadge v-if="expired" variant="destructive" class="self-start">
          {{ t('passwords.expiredOn', { date: detail.expiresAt }) }}
        </ShadcnBadge>

        <SettingsGroup>
          <SettingsRow
            v-if="detail.username"
            :title="t('passwords.fields.username')"
          >
            <span
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
                copyAsync({ kind: 'username' }, t('passwords.fields.username'))
              "
            >
              <Icon name="lucide:copy" class="size-4" />
            </UiButton>
          </SettingsRow>
          <SettingsRow :title="t('passwords.fields.password')">
            <PasswordsMaskedValue
              :fetch="
                async () =>
                  (await revealAsync(itemId, { kind: 'password' })).value
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
                copyAsync({ kind: 'password' }, t('passwords.fields.password'))
              "
            >
              <Icon name="lucide:copy" class="size-4" />
            </UiButton>
          </SettingsRow>
          <SettingsRow v-if="detail.url" :title="t('passwords.fields.url')">
            <span class="min-w-0 truncate" data-testid="passwords-value-url">{{
              detail.url
            }}</span>
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
          <li class="px-4 py-3">
            <PasswordsTotpCode
              :item-id="itemId"
              :state="detail.otpState"
              @copy="copyAsync({ kind: 'totp' }, t('passwords.fields.totp'))"
              @replace="edit"
              @remove="removeOtpAsync"
            />
          </li>
        </SettingsGroup>

        <SettingsGroup
          v-if="detail.keyValues.length"
          :label="t('passwords.fields.custom')"
        >
          <SettingsRow
            v-for="field in detail.keyValues"
            :key="field.id"
            :title="field.key ?? ''"
          >
            <PasswordsMaskedValue
              :fetch="
                async () =>
                  (
                    await revealAsync(itemId, {
                      kind: 'keyValue',
                      id: field.id,
                    })
                  ).value
              "
              :identity="`${itemId}:${field.id}`"
              kind="keyValue"
              :present="field.hasValue"
              :label="field.key ?? ''"
            />
            <UiButton
              v-if="field.hasValue"
              variant="ghost"
              size="icon"
              :aria-label="t('passwords.copy', { field: field.key ?? '' })"
              @click="
                copyAsync({ kind: 'keyValue', id: field.id }, field.key ?? '')
              "
            >
              <Icon name="lucide:copy" class="size-4" />
            </UiButton>
          </SettingsRow>
        </SettingsGroup>

        <SettingsGroup v-if="detail.note" :label="t('passwords.fields.note')">
          <li
            class="px-4 py-3 text-sm whitespace-pre-wrap"
            data-testid="passwords-value-note"
          >
            {{ detail.note }}
          </li>
        </SettingsGroup>

        <div
          v-if="detail.tags.length"
          class="flex flex-wrap gap-1.5"
          data-testid="passwords-entry-tags"
        >
          <ShadcnBadge
            v-for="tag in detail.tags"
            :key="tag.id"
            variant="secondary"
          >
            {{ tag.name }}
          </ShadcnBadge>
        </div>

        <PasswordsPasskeys
          :item-id="itemId"
          :passkeys="detail.passkeys"
          @changed="loadAsync"
        />
      </template>
    </div>
  </div>
</template>
