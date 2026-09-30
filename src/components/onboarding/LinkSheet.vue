<script setup lang="ts">
/**
 * "Mit einer Vault verknüpfen" on the landing page (spec 024, user story 5, FR-023 to FR-025): the
 * new installation enters the code a main device shows, gives this device a name and the new vault
 * its own passphrase. The vault is created here and stays only when the link finishes; nothing is
 * created before the user chose to link, so there is no vault to leave behind (FR-025). The
 * passphrase never leaves this device.
 */
import type { LinkJoinState } from '@bindings/LinkJoinState'

interface Props {
  open: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  /** The link finished; the vault exists and can be unlocked. */
  linked: [name: string]
}>()

const { t } = useI18n()
const passwordLabels = computed(() => ({
  show: t('onboarding.passwordField.show'),
  hide: t('onboarding.passwordField.hide'),
  copy: t('onboarding.passwordField.copy'),
  copied: t('onboarding.passwordField.copied'),
}))
const { joinStartAsync, joinCancelAsync, onJoinState } = useDeviceLink()

const code = ref('')
const vaultName = ref('')
const deviceName = ref('')
const passphrase = ref('')
const passphraseConfirm = ref('')
const submitting = ref(false)
const error = ref<string | null>(null)
/** `null` while the form is shown. */
const state = ref<LinkJoinState | null>(null)

const running = computed(
  () =>
    state.value !== null &&
    state.value.state !== 'done' &&
    state.value.state !== 'failed',
)

function reset() {
  code.value = ''
  vaultName.value = ''
  deviceName.value = ''
  passphrase.value = ''
  passphraseConfirm.value = ''
  error.value = null
  state.value = null
}

let stopListening: (() => void) | undefined
onMounted(async () => {
  stopListening = await onJoinState((next) => {
    if (state.value !== null) state.value = next
  })
})
onBeforeUnmount(() => stopListening?.())

// Closing the sheet ends a link in progress; its vault is removed (FR-025).
let closeRequested = false
let cancelOnClose: Promise<void> | undefined

watch(
  () => props.open,
  (isOpen) => {
    closeRequested = !isOpen
    if (isOpen) return
    if (running.value) {
      cancelOnClose = joinCancelAsync().finally(() => {
        cancelOnClose = undefined
        if (!props.open) reset()
      })
      return
    }
    reset()
  },
)

const canSubmit = computed(
  () =>
    code.value.trim().length > 0 &&
    vaultName.value.length > 0 &&
    deviceName.value.trim().length > 0 &&
    passphrase.value.length >= 8 &&
    passphrase.value === passphraseConfirm.value &&
    !submitting.value,
)

async function onSubmit() {
  if (!canSubmit.value) return
  submitting.value = true
  error.value = null
  // Shown from the first moment, so an event that arrives before `joinStartAsync` returns is not lost.
  state.value = { state: 'searching' }
  try {
    const started = await joinStartAsync({
      code: code.value,
      vaultName: vaultName.value,
      deviceName: deviceName.value.trim(),
      passphrase: passphrase.value,
    })
    if (closeRequested) {
      await cancelOnClose
      await joinCancelAsync()
      return
    }
    // An event may already have moved on; only the first state is replaced.
    if (state.value?.state === 'searching') state.value = started
    // The passphrase is only needed to create the vault; do not keep it in the form.
    passphrase.value = ''
    passphraseConfirm.value = ''
  } catch (e: unknown) {
    state.value = null
    // The backend names error kinds in PascalCase; the texts are keyed in camelCase.
    const kind = (e as { kind?: string })?.kind
    const key = kind
      ? kind.charAt(0).toLowerCase() + kind.slice(1)
      : 'openFailed'
    error.value = t(`errors.${key}`, t('errors.openFailed'))
  } finally {
    submitting.value = false
  }
}

async function onCancel() {
  await joinCancelAsync()
}

/** Back to the form after a failure. */
function onRetry() {
  state.value = null
  error.value = null
}

function onOpenVault() {
  if (state.value?.state !== 'done') return
  const name = state.value.vaultName
  emit('update:open', false)
  emit('linked', name)
}
</script>

<template>
  <UiDrawerModal
    :open="props.open"
    :title="t('onboarding.link.title')"
    @update:open="$emit('update:open', $event)"
  >
    <template #content>
      <form
        v-if="state === null"
        id="link-form"
        class="space-y-4 px-6 py-2"
        @submit.prevent="onSubmit"
      >
        <p class="text-sm text-muted-foreground">
          {{ t('onboarding.link.intro') }}
        </p>
        <div class="space-y-1.5">
          <ShadcnLabel for="link-code">
            {{ t('onboarding.link.code') }}
          </ShadcnLabel>
          <ShadcnInput
            id="link-code"
            v-model="code"
            autofocus
            autocomplete="off"
            spellcheck="false"
            class="font-mono uppercase"
            :placeholder="t('onboarding.link.codePlaceholder')"
            data-testid="link-code"
          />
        </div>
        <div class="space-y-1.5">
          <ShadcnLabel for="link-device-name">
            {{ t('onboarding.link.deviceName') }}
          </ShadcnLabel>
          <ShadcnInput
            id="link-device-name"
            v-model="deviceName"
            data-testid="link-device-name"
          />
        </div>
        <div class="space-y-1.5">
          <ShadcnLabel for="link-vault-name">
            {{ t('onboarding.link.vaultName') }}
          </ShadcnLabel>
          <ShadcnInput
            id="link-vault-name"
            v-model="vaultName"
            data-testid="link-vault-name"
          />
        </div>
        <div class="space-y-1.5">
          <ShadcnLabel for="link-passphrase">
            {{ t('onboarding.link.passphrase') }}
          </ShadcnLabel>
          <UiInputPassword
            id="link-passphrase"
            v-model="passphrase"
            :labels="passwordLabels"
          />
          <p class="text-xs text-muted-foreground">
            {{ t('onboarding.link.passphraseHint') }}
          </p>
        </div>
        <div class="space-y-1.5">
          <ShadcnLabel for="link-passphrase-confirm">
            {{ t('onboarding.create.passphraseConfirm') }}
          </ShadcnLabel>
          <UiInputPassword
            id="link-passphrase-confirm"
            v-model="passphraseConfirm"
            :labels="passwordLabels"
          />
        </div>
        <p v-if="error" class="text-sm text-destructive" role="alert">
          {{ error }}
        </p>
      </form>

      <div
        v-else
        class="flex flex-col items-center gap-3 px-6 py-6 text-center"
        role="status"
        data-testid="link-progress"
        :data-state="state.state"
      >
        <template v-if="running">
          <Icon name="lucide:loader-circle" class="size-8 animate-spin" />
          <p class="font-medium">
            {{ t(`onboarding.link.state.${state.state}`) }}
          </p>
          <p
            v-if="state.state === 'searching'"
            class="text-sm text-muted-foreground"
          >
            {{ t('onboarding.link.searchingHint') }}
          </p>
        </template>
        <template v-else-if="state.state === 'done'">
          <Icon name="lucide:circle-check" class="size-8 text-primary" />
          <p class="font-medium">
            {{ t('onboarding.link.done', { name: state.vaultName }) }}
          </p>
          <p class="text-sm text-muted-foreground">
            {{ t('onboarding.link.doneHint') }}
          </p>
        </template>
        <template v-else-if="state.state === 'failed'">
          <Icon name="lucide:circle-alert" class="size-8 text-destructive" />
          <p class="font-medium" role="alert">
            {{ t(`onboarding.link.failed.${state.reason}`) }}
          </p>
          <p class="text-sm text-muted-foreground">
            {{ t('onboarding.link.failedHint') }}
          </p>
        </template>
      </div>
    </template>
    <template #footer>
      <UiButton
        v-if="state === null"
        form="link-form"
        type="submit"
        :disabled="!canSubmit"
        :loading="submitting"
        class="w-full"
      >
        {{ t('onboarding.link.submit') }}
      </UiButton>
      <UiButton
        v-else-if="running"
        variant="outline"
        class="w-full"
        @click="onCancel"
      >
        {{ t('onboarding.link.cancel') }}
      </UiButton>
      <UiButton
        v-else-if="state.state === 'done'"
        class="w-full"
        data-testid="link-open-vault"
        @click="onOpenVault"
      >
        {{ t('onboarding.link.openVault') }}
      </UiButton>
      <UiButton v-else class="w-full" @click="onRetry">
        {{ t('onboarding.link.retry') }}
      </UiButton>
    </template>
  </UiDrawerModal>
</template>
