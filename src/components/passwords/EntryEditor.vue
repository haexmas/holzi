<script setup lang="ts">
/**
 * The editor of an entry (spec 034, US1, FR-001..FR-003, FR-005, research R7 and R15): create at
 * `/entry/new`, edit at `/entry/:id?edit`. The title is optional. The password area shows
 * `••••` with "Ersetzen" for a stored entry and sends a password only when it was replaced; the
 * custom fields likewise keep their stored value until the user types a new one. An invalid TOTP
 * value is refused with a message at its field. A save that finds the entry changed or deleted
 * meanwhile asks what to do instead of overwriting (`ConflictDialog`), and leaving with unsaved
 * changes asks too (`UnsavedDialog`). The draft is a local object that the quiet reload of the store
 * never touches.
 */
import { toast } from 'vue-sonner'
import type { ItemDetail } from '@bindings/ItemDetail'
import {
  cloneDraft,
  draftFromDetail,
  emptyDraft,
  isDirty,
  toInput,
  toPatch,
  type Draft,
} from '~/lib/passwords/draft'
import { ENTRY_COLORS, ENTRY_ICONS } from '~/lib/passwords/icons'

const props = defineProps<{
  /** `null` creates a new entry. */
  itemId: string | null
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const router = useTabRouter()
const tab = useWmTab()
const store = usePasswordsStore()
const { getItemAsync, createItemAsync, updateItemAsync, revealAsync } =
  usePasswords()

const detail = ref<ItemDetail | null>(null)
const initial = ref<Draft>(emptyDraft())
const draft = ref<Draft>(emptyDraft())
const loading = ref(props.itemId !== null)
const loadError = ref<string | null>(null)
const saving = ref(false)
const otpError = ref<string | null>(null)
const saveError = ref<string | null>(null)
const conflict = ref<'changed' | 'deleted' | null>(null)
const askingToLeave = ref(false)
const tagInput = ref('')
const replacingOtp = ref(false)

const isNew = computed(() => props.itemId === null)
const dirty = computed(() => isDirty(initial.value, draft.value))

async function loadAsync() {
  if (props.itemId === null) return
  loading.value = true
  try {
    detail.value = await getItemAsync(props.itemId)
    initial.value = draftFromDetail(detail.value)
    draft.value = cloneDraft(initial.value)
    loadError.value = null
  } catch (cause) {
    loadError.value = errString(cause)
  } finally {
    loading.value = false
  }
}

onMounted(loadAsync)

// Leaving the tab or window with unsaved changes is guarded by the window manager.
const unregister = tab.registerCloseGuard(() =>
  dirty.value
    ? {
        reasonKey: 'passwords.unsaved.reason',
        confirmAsync: async () => {},
      }
    : null,
)
onBeforeUnmount(() => unregister())

function viewPath(id: string): string {
  return `/entry/${id}`
}

function leave() {
  if (props.itemId !== null) router.replace(viewPath(props.itemId))
  else if (router.canGoBack) router.back()
  else router.replace('/')
}

function requestLeave() {
  if (dirty.value) askingToLeave.value = true
  else leave()
}

const OTP_FIELD_ERRORS = new Set([
  'otpSecret',
  'otpDigits',
  'otpPeriod',
  'otpAlgorithm',
])

async function saveAsync(): Promise<boolean> {
  saving.value = true
  otpError.value = null
  saveError.value = null
  try {
    if (props.itemId === null) {
      const created = await createItemAsync(toInput(draft.value))
      await store.quietReloadAsync()
      router.replace(viewPath(created.itemId))
      return true
    }
    const patch = toPatch(initial.value, draft.value)
    if (Object.keys(patch).length > 0) {
      const updatedAt = detail.value?.updatedAt
      if (!updatedAt) throw new Error('missing update token')
      await updateItemAsync(props.itemId, updatedAt, patch)
      await store.quietReloadAsync()
    }
    // Reset the baseline so the guard does not fire on the way out.
    initial.value = cloneDraft(draft.value)
    router.replace(viewPath(props.itemId))
    return true
  } catch (cause) {
    const error = cause as { kind?: string; reason?: string }
    if (
      error.kind === 'InvalidInput' &&
      OTP_FIELD_ERRORS.has(error.reason ?? '')
    ) {
      otpError.value = t(`passwords.editor.otpErrors.${error.reason}`)
    } else if (error.kind === 'PasswordsConflict') {
      conflict.value = error.reason === 'deleted' ? 'deleted' : 'changed'
    } else {
      saveError.value = errString(cause)
    }
    return false
  } finally {
    saving.value = false
  }
}

async function saveAndLeaveAsync() {
  askingToLeave.value = false
  await saveAsync()
}

function discardAndLeave() {
  askingToLeave.value = false
  draft.value = cloneDraft(initial.value)
  leave()
}

/** The draft with the stored values of everything the user did not touch, so it can become a new
 * entry. A deleted entry cannot be read any more; those values are then not part of it. */
async function materialiseAsync(): Promise<Draft> {
  const filled = cloneDraft(draft.value)
  const current = detail.value
  if (props.itemId === null || !current || conflict.value === 'deleted')
    return filled
  if (filled.password.mode === 'keep' && current.hasPassword) {
    const revealed = await revealAsync(props.itemId, { kind: 'password' })
    filled.password = { mode: 'set', value: revealed.value }
  }
  if (filled.otp.mode === 'keep' && current.hasOtpSecret) {
    const revealed = await revealAsync(props.itemId, { kind: 'otpSecret' })
    filled.otp = {
      mode: 'set',
      text: revealed.value,
      digits: current.otpDigits,
      period: current.otpPeriod,
      algorithm: current.otpAlgorithm,
    }
  }
  for (const field of filled.keyValues) {
    if (field.id !== null && field.value === null && field.hasStoredValue) {
      const revealed = await revealAsync(props.itemId, {
        kind: 'keyValue',
        id: field.id,
      })
      field.value = revealed.value
    }
  }
  return filled
}

async function saveAsNewAsync() {
  saving.value = true
  try {
    const created = await createItemAsync(toInput(await materialiseAsync()))
    conflict.value = null
    initial.value = cloneDraft(draft.value)
    await store.quietReloadAsync()
    router.replace(viewPath(created.itemId))
  } catch (cause) {
    conflict.value = null
    saveError.value = errString(cause)
  } finally {
    saving.value = false
  }
}

async function takeCurrentAsync() {
  conflict.value = null
  await loadAsync()
  if (props.itemId) router.replace(viewPath(props.itemId))
}

function addTag() {
  const name = tagInput.value.trim()
  tagInput.value = ''
  if (!name) return
  const known = draft.value.tags.some(
    (tag) => tag.toLowerCase() === name.toLowerCase(),
  )
  if (!known) draft.value.tags = [...draft.value.tags, name]
}

function removeTag(name: string) {
  draft.value.tags = draft.value.tags.filter((tag) => tag !== name)
}

function startReplacingPassword() {
  draft.value.password = { mode: 'set', value: '' }
}

function startReplacingOtp() {
  replacingOtp.value = true
  draft.value.otp = {
    mode: 'set',
    text: '',
    digits: null,
    period: null,
    algorithm: null,
  }
}

function removeOtp() {
  replacingOtp.value = false
  draft.value.otp = { mode: 'clear' }
}

function otpText(): string {
  return draft.value.otp.mode === 'set' ? draft.value.otp.text : ''
}

function setOtpText(text: string) {
  if (draft.value.otp.mode === 'set') draft.value.otp.text = text
  else
    draft.value.otp = {
      mode: 'set',
      text,
      digits: null,
      period: null,
      algorithm: null,
    }
}

const passwordValue = computed({
  get: () =>
    draft.value.password.mode === 'set' ? draft.value.password.value : '',
  set: (value: string) => {
    draft.value.password = { mode: 'set', value }
  },
})

const showOtpInput = computed(
  () => isNew.value || !detail.value?.hasOtpSecret || replacingOtp.value,
)

watch(saveError, (message) => {
  if (message) toast.error(message)
})
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-6 @md:px-6">
    <form
      class="mx-auto flex w-full max-w-3xl flex-col gap-5"
      data-testid="passwords-editor"
      @submit.prevent="saveAsync"
    >
      <div class="flex items-center gap-2 pt-1">
        <UiButton
          type="button"
          variant="ghost"
          size="icon"
          class="-ml-2 shrink-0"
          :aria-label="t('passwords.cancel')"
          :tooltip="t('passwords.cancel')"
          data-testid="passwords-editor-cancel"
          @click="requestLeave"
        >
          <Icon name="lucide:arrow-left" class="size-5" />
        </UiButton>
        <h1 class="min-w-0 flex-1 truncate text-2xl font-bold">
          {{
            isNew
              ? t('passwords.editor.newTitle')
              : t('passwords.editor.editTitle')
          }}
        </h1>
        <UiButton
          type="submit"
          class="shrink-0"
          :loading="saving"
          :disabled="loading || (!isNew && !dirty)"
          data-testid="passwords-editor-save"
        >
          {{ t('passwords.save') }}
        </UiButton>
      </div>

      <p v-if="loadError" class="text-sm text-destructive" role="alert">
        {{ loadError }}
      </p>
      <div
        v-else-if="loading"
        class="flex justify-center py-10 text-muted-foreground"
        role="status"
      >
        <Icon name="lucide:loader-circle" class="size-6 animate-spin" />
      </div>

      <template v-else>
        <SettingsGroup>
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <ShadcnLabel for="pw-title">{{
              t('passwords.fields.title')
            }}</ShadcnLabel>
            <ShadcnInput
              id="pw-title"
              v-model="draft.title"
              :placeholder="t('passwords.untitled')"
              data-testid="passwords-field-title"
            />
          </li>
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <ShadcnLabel for="pw-username">{{
              t('passwords.fields.username')
            }}</ShadcnLabel>
            <ShadcnInput
              id="pw-username"
              v-model="draft.username"
              autocomplete="off"
              data-testid="passwords-field-username"
            />
          </li>
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <ShadcnLabel for="pw-password">{{
              t('passwords.fields.password')
            }}</ShadcnLabel>
            <div
              v-if="draft.password.mode === 'keep'"
              class="flex items-center gap-2"
            >
              <PasswordsMaskedValue
                v-if="itemId"
                :item-id="itemId"
                :field="{ kind: 'password' }"
                :present="detail?.hasPassword ?? false"
                :label="t('passwords.fields.password')"
                class="flex-1"
              />
              <UiButton
                type="button"
                variant="outline"
                size="sm"
                data-testid="passwords-replace-password"
                @click="startReplacingPassword"
              >
                {{ t('passwords.editor.replace') }}
              </UiButton>
            </div>
            <UiInputPassword
              v-else
              id="pw-password"
              v-model="passwordValue"
              autocomplete="new-password"
              data-testid="passwords-field-password"
            />
          </li>
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <ShadcnLabel for="pw-url">{{
              t('passwords.fields.url')
            }}</ShadcnLabel>
            <ShadcnInput
              id="pw-url"
              v-model="draft.url"
              type="url"
              inputmode="url"
              data-testid="passwords-field-url"
            />
          </li>
        </SettingsGroup>

        <SettingsGroup :label="t('passwords.fields.totp')">
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <template v-if="showOtpInput">
              <ShadcnLabel for="pw-otp">{{
                t('passwords.editor.otpLabel')
              }}</ShadcnLabel>
              <ShadcnInput
                id="pw-otp"
                :model-value="otpText()"
                autocomplete="off"
                spellcheck="false"
                :placeholder="t('passwords.editor.otpPlaceholder')"
                :aria-invalid="otpError !== null"
                data-testid="passwords-field-otp"
                @update:model-value="setOtpText(String($event))"
              />
              <p
                v-if="otpError"
                class="text-sm text-destructive"
                role="alert"
                data-testid="passwords-otp-error"
              >
                {{ otpError }}
              </p>
            </template>
            <div v-else class="flex flex-wrap items-center gap-2">
              <span
                class="min-w-0 flex-1 text-sm"
                :class="
                  detail?.otpState === 'invalid' ? 'text-destructive' : ''
                "
              >
                {{
                  detail?.otpState === 'invalid'
                    ? t('passwords.totp.invalid')
                    : t('passwords.editor.otpSet')
                }}
              </span>
              <UiButton
                type="button"
                variant="outline"
                size="sm"
                data-testid="passwords-replace-otp"
                @click="startReplacingOtp"
              >
                {{ t('passwords.editor.replace') }}
              </UiButton>
              <UiButton
                type="button"
                variant="outline"
                size="sm"
                data-testid="passwords-remove-otp"
                @click="removeOtp"
              >
                {{ t('passwords.totp.remove') }}
              </UiButton>
            </div>
            <p
              v-if="draft.otp.mode === 'clear'"
              class="text-sm text-muted-foreground"
            >
              {{ t('passwords.editor.otpWillBeRemoved') }}
            </p>
          </li>
        </SettingsGroup>

        <SettingsGroup :label="t('passwords.fields.custom')">
          <li class="px-4 py-3">
            <PasswordsKeyValues v-model="draft.keyValues" />
          </li>
        </SettingsGroup>

        <SettingsGroup>
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <ShadcnLabel for="pw-note">{{
              t('passwords.fields.note')
            }}</ShadcnLabel>
            <ShadcnTextarea
              id="pw-note"
              v-model="draft.note"
              rows="4"
              data-testid="passwords-field-note"
            />
          </li>
          <li class="flex flex-col gap-1.5 px-4 py-3">
            <ShadcnLabel for="pw-expires">{{
              t('passwords.fields.expires')
            }}</ShadcnLabel>
            <ShadcnInput
              id="pw-expires"
              v-model="draft.expiresAt"
              type="date"
              class="w-48"
              data-testid="passwords-field-expires"
            />
          </li>
          <li class="flex flex-col gap-2 px-4 py-3">
            <ShadcnLabel for="pw-tag">{{
              t('passwords.fields.tags')
            }}</ShadcnLabel>
            <div v-if="draft.tags.length" class="flex flex-wrap gap-1.5">
              <ShadcnBadge
                v-for="tag in draft.tags"
                :key="tag"
                variant="secondary"
                class="gap-1"
              >
                {{ tag }}
                <button
                  type="button"
                  class="rounded-full hover:text-destructive"
                  :aria-label="t('passwords.editor.removeTag', { tag })"
                  @click="removeTag(tag)"
                >
                  <Icon name="lucide:x" class="size-3" />
                </button>
              </ShadcnBadge>
            </div>
            <ShadcnInput
              id="pw-tag"
              v-model="tagInput"
              :placeholder="t('passwords.editor.tagPlaceholder')"
              data-testid="passwords-field-tag"
              @keydown.enter.prevent="addTag"
              @blur="addTag"
            />
          </li>
        </SettingsGroup>

        <SettingsGroup :label="t('passwords.editor.look')">
          <li class="flex flex-col gap-3 px-4 py-3">
            <div
              class="flex flex-wrap gap-1.5"
              role="radiogroup"
              :aria-label="t('passwords.fields.icon')"
            >
              <button
                v-for="name in ENTRY_ICONS"
                :key="name"
                type="button"
                role="radio"
                :aria-checked="draft.icon === name"
                :aria-label="name.replace('lucide:', '')"
                class="flex size-9 items-center justify-center rounded-lg border"
                :class="
                  draft.icon === name
                    ? 'border-primary bg-primary/10'
                    : 'border-transparent bg-background hover:bg-accent'
                "
                @click="draft.icon = draft.icon === name ? null : name"
              >
                <Icon :name="name" class="size-5" />
              </button>
            </div>
            <div
              class="flex flex-wrap gap-1.5"
              role="radiogroup"
              :aria-label="t('passwords.fields.color')"
            >
              <button
                v-for="color in ENTRY_COLORS"
                :key="color"
                type="button"
                role="radio"
                :aria-checked="draft.color === color"
                :aria-label="color"
                class="size-7 rounded-full border-2"
                :class="
                  draft.color === color
                    ? 'border-foreground'
                    : 'border-transparent'
                "
                :style="{ backgroundColor: color }"
                @click="draft.color = draft.color === color ? null : color"
              />
            </div>
          </li>
        </SettingsGroup>
      </template>
    </form>

    <PasswordsConflictDialog
      :open="conflict !== null"
      :reason="conflict ?? 'changed'"
      @save-as-new="saveAsNewAsync"
      @take-current="takeCurrentAsync"
      @close="conflict = null"
    />
    <PasswordsUnsavedDialog
      :open="askingToLeave"
      @save="saveAndLeaveAsync"
      @discard="discardAndLeave"
      @stay="askingToLeave = false"
    />
  </div>
</template>
