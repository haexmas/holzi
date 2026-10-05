<script setup lang="ts">
/**
 * The dialog before a copy is pasted (spec 036, US3, FR-015): a dialog on wide windows, a drawer
 * on narrow ones. For one entry its title (default "<Titel> – Kopie" or "– Copy" by the interface
 * language), for several entries or a folder a common suffix; "Verlauf übernehmen", user name or
 * password as a reference on the original and "Passkeys per Verweis" (research R6), all off by
 * default. Cancel creates nothing. The copy runs in one transaction; the Ablage stays filled, so
 * it can be pasted again.
 */
import { toast } from 'vue-sonner'
import type { CopyRequest } from '~/composables/usePasswordsActions'
import { displayTitle } from '~/lib/passwords/format'

const props = defineProps<{
  request: CopyRequest | null
}>()

const open = defineModel<boolean>('open', { required: true })

const emit = defineEmits<{
  done: []
}>()

const { t } = useI18n()
const fieldLabels = useFieldLabels()
const { errString } = useErrorString()
const store = usePasswordsStore()
const { copyAsync } = usePasswords()

const title = ref('')
const suffix = ref('')
const history = ref(false)
const usernameAsReference = ref(false)
const passwordAsReference = ref(false)
/** A passkey is never copied; this shows the original's passkeys at the copy by a link. */
const passkeysAsLinks = ref(false)
const busy = ref(false)

/** One entry gets an exact title; several or a folder a suffix. */
const single = computed(() => {
  const targets = props.request?.targets ?? []
  return targets.length === 1 && targets[0]?.kind === 'item' ? targets[0] : null
})

watch(open, (isOpen) => {
  if (!isOpen) return
  const defaultSuffix = t('passwords.copyDialog.suffix')
  suffix.value = defaultSuffix
  const entry = single.value ? store.headersById.get(single.value.id) : null
  title.value = `${displayTitle(entry?.title) ?? t('passwords.untitled')}${defaultSuffix}`
  history.value = false
  usernameAsReference.value = false
  passwordAsReference.value = false
  passkeysAsLinks.value = false
})

async function confirmAsync() {
  const request = props.request
  // Enter in a field submits the form again while a copy runs; a second copy would duplicate.
  if (!request || busy.value) return
  busy.value = true
  try {
    const report = await copyAsync(request.targets, request.into, {
      title: single.value ? { exact: title.value } : { suffix: suffix.value },
      history: history.value,
      usernameAsReference: usernameAsReference.value,
      passwordAsReference: passwordAsReference.value,
      passkeysAsLinks: passkeysAsLinks.value,
    })
    await store.quietReloadAsync()
    // Empty folders copy no entry; "0 Einträge kopiert" would read as a failure.
    const created =
      report.itemsCreated === 0 && report.groupsCreated > 0
        ? t(
            'passwords.copyDialog.createdFolders',
            { count: report.groupsCreated },
            report.groupsCreated,
          )
        : t(
            'passwords.copyDialog.created',
            { count: report.itemsCreated },
            report.itemsCreated,
          )
    toast.success(
      report.skippedMissing > 0
        ? t('passwords.clipboard.movedSome', {
            moved: created,
            missing: t(
              'passwords.clipboard.missingCount',
              { count: report.skippedMissing },
              report.skippedMissing,
            ),
          })
        : created,
    )
    open.value = false
    emit('done')
  } catch (cause) {
    toast.error(errString(cause))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UiDrawerModal v-model:open="open" :title="t('passwords.copyDialog.title')">
    <template #content>
      <form
        class="flex flex-col gap-4"
        data-testid="passwords-copy-dialog"
        @submit.prevent="confirmAsync"
      >
        <UiInput
          v-if="single"
          v-model="title"
          :label="t('passwords.fields.title')"
          :labels="fieldLabels.input.value"
          data-testid="passwords-copy-title"
        />
        <UiInput
          v-else
          v-model="suffix"
          :label="t('passwords.copyDialog.suffixLabel')"
          :labels="fieldLabels.input.value"
          data-testid="passwords-copy-suffix"
        />
        <label class="flex items-start gap-2 text-sm">
          <ShadcnCheckbox
            v-model="history"
            class="mt-0.5"
            data-testid="passwords-copy-history"
          />
          <span>
            {{ t('passwords.copyDialog.history') }}
            <span class="block text-muted-foreground">{{
              t('passwords.copyDialog.historyHint')
            }}</span>
          </span>
        </label>
        <label class="flex items-start gap-2 text-sm">
          <ShadcnCheckbox
            v-model="usernameAsReference"
            class="mt-0.5"
            data-testid="passwords-copy-username-reference"
          />
          <span>{{ t('passwords.copyDialog.usernameReference') }}</span>
        </label>
        <label class="flex items-start gap-2 text-sm">
          <ShadcnCheckbox
            v-model="passwordAsReference"
            class="mt-0.5"
            data-testid="passwords-copy-password-reference"
          />
          <span>{{ t('passwords.copyDialog.passwordReference') }}</span>
        </label>
        <label class="flex items-start gap-2 text-sm">
          <ShadcnCheckbox
            v-model="passkeysAsLinks"
            class="mt-0.5"
            data-testid="passwords-copy-passkey-links"
          />
          <span>
            {{ t('passwords.copyDialog.passkeyLinks') }}
            <span class="block text-muted-foreground">{{
              t('passwords.copyDialog.noPasskeys')
            }}</span>
          </span>
        </label>
      </form>
    </template>
    <template #footer>
      <UiButton
        variant="outline"
        data-testid="passwords-copy-cancel"
        @click="open = false"
      >
        {{ t('passwords.cancel') }}
      </UiButton>
      <UiButton
        :loading="busy"
        data-testid="passwords-copy-confirm"
        @click="confirmAsync"
      >
        {{ t('passwords.copyDialog.confirm') }}
      </UiButton>
    </template>
  </UiDrawerModal>
</template>
