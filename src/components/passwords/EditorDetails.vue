<script setup lang="ts">
/**
 * The tab Details of the editor (spec 036, FR-001, FR-004; fields from spec 034 FR-001..FR-003):
 * title, username, password, address, TOTP, note, expiry, tags, icon and colour. The password
 * shows `••••` with "Ersetzen" for a stored entry and is sent only when it was replaced; an
 * invalid TOTP value is refused at its field (`otpError`). The generator opens in a drawer. The
 * draft is the parent's object; this tab only edits it.
 */
import type { ItemDetail } from '@bindings/ItemDetail'
import type { Draft } from '~/lib/passwords/draft'
import { ENTRY_COLORS, ENTRY_ICONS } from '~/lib/passwords/icons'

const props = defineProps<{
  /** `null` is a new entry. */
  itemId: string | null
  detail: ItemDetail | null
  otpError: string | null
}>()
const draft = defineModel<Draft>({ required: true })

const { t } = useI18n()
const fieldLabels = useFieldLabels()
const { revealAsync } = usePasswords()

const tagInput = ref('')
const replacingOtp = ref(false)
const generatorOpen = ref(false)

const isNew = computed(() => props.itemId === null)

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
  () => isNew.value || !props.detail?.hasOtpSecret || replacingOtp.value,
)
</script>

<template>
  <div class="flex flex-col gap-5" data-testid="entry-editor-details">
    <SettingsGroup>
      <li class="px-4 py-3">
        <UiInput
          id="pw-title"
          v-model="draft.title"
          :label="t('passwords.fields.title')"
          :placeholder="t('passwords.untitled')"
          :labels="fieldLabels.input.value"
          label-bg="var(--muted)"
          data-testid="passwords-field-title"
        />
      </li>
      <li class="px-4 py-3">
        <UiInput
          id="pw-username"
          v-model="draft.username"
          :label="t('passwords.fields.username')"
          :labels="fieldLabels.input.value"
          label-bg="var(--muted)"
          autocomplete="off"
          copyable
          data-testid="passwords-field-username"
        />
        <PasswordsReferenceField
          class="mt-1.5"
          :text="draft.username"
          :item-id="itemId"
          kind="username"
          @update:text="draft.username = $event"
        />
      </li>
      <li class="flex flex-col gap-1.5 px-4 py-3">
        <ShadcnLabel v-if="draft.password.mode === 'keep'" for="pw-password">{{
          t('passwords.fields.password')
        }}</ShadcnLabel>
        <div
          v-if="draft.password.mode === 'keep'"
          class="flex items-center gap-2"
        >
          <PasswordsMaskedValue
            v-if="itemId"
            :fetch="
              async () =>
                (await revealAsync(itemId!, { kind: 'password' })).value
            "
            :identity="`${itemId}:password`"
            kind="password"
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
        <div v-else class="flex items-center gap-2">
          <UiInputPassword
            id="pw-password"
            v-model="passwordValue"
            :label="t('passwords.fields.password')"
            :labels="fieldLabels.password.value"
            label-bg="var(--muted)"
            autocomplete="new-password"
            class="flex-1"
            data-testid="passwords-field-password"
          />
          <UiButton
            type="button"
            variant="outline"
            size="sm"
            data-testid="passwords-generate"
            @click="generatorOpen = true"
          >
            <Icon name="lucide:wand-sparkles" class="size-4" />
            {{ t('passwords.generator.open') }}
          </UiButton>
        </div>
        <PasswordsReferenceField
          :text="draft.password.mode === 'set' ? draft.password.value : null"
          :stored-marks="detail?.references.password"
          :item-id="itemId"
          kind="password"
          @update:text="draft.password = { mode: 'set', value: $event }"
        />
      </li>
      <li class="px-4 py-3">
        <UiInput
          id="pw-url"
          v-model="draft.url"
          :label="t('passwords.fields.url')"
          :labels="fieldLabels.input.value"
          label-bg="var(--muted)"
          type="url"
          inputmode="url"
          copyable
          data-testid="passwords-field-url"
        />
        <PasswordsReferenceField
          class="mt-1.5"
          :text="draft.url"
          :item-id="itemId"
          kind="url"
          @update:text="draft.url = $event"
        />
      </li>
    </SettingsGroup>

    <SettingsGroup :label="t('passwords.fields.totp')">
      <li class="flex flex-col gap-1.5 px-4 py-3">
        <UiInput
          v-if="showOtpInput"
          id="pw-otp"
          :model-value="otpText()"
          :label="t('passwords.editor.otpLabel')"
          :labels="fieldLabels.input.value"
          label-bg="var(--muted)"
          :error="otpError ?? undefined"
          autocomplete="off"
          spellcheck="false"
          :placeholder="t('passwords.editor.otpPlaceholder')"
          data-testid="passwords-field-otp"
          @update:model-value="setOtpText(String($event ?? ''))"
        />
        <div v-else class="flex flex-wrap items-center gap-2">
          <span
            class="min-w-0 flex-1 text-sm"
            :class="detail?.otpState === 'invalid' ? 'text-destructive' : ''"
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

    <SettingsGroup>
      <li class="px-4 py-3">
        <UiTextarea
          id="pw-note"
          v-model="draft.note"
          :label="t('passwords.fields.note')"
          label-bg="var(--muted)"
          rows="4"
          data-testid="passwords-field-note"
        />
        <PasswordsReferenceField
          class="mt-1.5"
          :text="draft.note"
          :item-id="itemId"
          kind="note"
          @update:text="draft.note = $event"
        />
      </li>
      <li class="px-4 py-3">
        <div class="w-48">
          <UiInput
            id="pw-expires"
            v-model="draft.expiresAt"
            :label="t('passwords.fields.expires')"
            :labels="fieldLabels.input.value"
            label-bg="var(--muted)"
            type="date"
            data-testid="passwords-field-expires"
          />
        </div>
      </li>
      <li class="flex flex-col gap-2 px-4 py-3">
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
        <UiInput
          id="pw-tag"
          v-model="tagInput"
          :label="t('passwords.fields.tags')"
          :labels="fieldLabels.input.value"
          label-bg="var(--muted)"
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
              draft.color === color ? 'border-foreground' : 'border-transparent'
            "
            :style="{ backgroundColor: color }"
            @click="draft.color = draft.color === color ? null : color"
          />
        </div>
      </li>
    </SettingsGroup>

    <UiDrawerModal
      v-model:open="generatorOpen"
      :title="t('passwords.generator.title')"
    >
      <template #content>
        <PasswordsGeneratorPanel
          embedded
          @use="
            (password: string) => {
              passwordValue = password
              generatorOpen = false
            }
          "
        />
      </template>
    </UiDrawerModal>
  </div>
</template>
