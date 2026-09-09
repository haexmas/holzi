<script setup lang="ts">
interface Props {
  open: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  created: [name: string]
}>()

const { t } = useI18n()
const { createAsync } = useInstance()

const name = ref('')
const passphrase = ref('')
const passphraseConfirm = ref('')
const submitting = ref(false)
const error = ref<string | null>(null)

function reset() {
  name.value = ''
  passphrase.value = ''
  passphraseConfirm.value = ''
  error.value = null
  submitting.value = false
}

watch(() => props.open, (isOpen) => {
  if (!isOpen)
    reset()
})

const canSubmit = computed(() =>
  name.value.length > 0
  && passphrase.value.length >= 8
  && passphrase.value === passphraseConfirm.value
  && !submitting.value,
)

async function onSubmit() {
  if (!canSubmit.value)
    return
  submitting.value = true
  error.value = null
  try {
    const result = await createAsync({
      name: name.value,
      passphrase: passphrase.value,
    })
    emit('created', result.info.name)
    emit('update:open', false)
  }
  catch (e: unknown) {
    const kind = (e as { kind?: string })?.kind
    error.value = t(`errors.${kind ?? 'openFailed'}`, t('errors.openFailed'))
  }
  finally {
    submitting.value = false
  }
}
</script>

<template>
  <UiSheet :open="props.open" :title="t('onboarding.create.title')" @update:open="$emit('update:open', $event)">
    <form class="space-y-4" @submit.prevent="onSubmit">
      <div>
        <UiLabel for="create-name">
          {{ t('onboarding.create.name') }}
        </UiLabel>
        <UiInput
          id="create-name"
          v-model="name"
          autofocus
        />
      </div>
      <div>
        <UiLabel for="create-passphrase">
          {{ t('onboarding.create.passphrase') }}
        </UiLabel>
        <UiInput
          id="create-passphrase"
          v-model="passphrase"
          type="password"
        />
      </div>
      <div>
        <UiLabel for="create-passphrase-confirm">
          {{ t('onboarding.create.passphraseConfirm') }}
        </UiLabel>
        <UiInput
          id="create-passphrase-confirm"
          v-model="passphraseConfirm"
          type="password"
        />
      </div>
      <p v-if="error" class="text-sm text-red-500" role="alert">
        {{ error }}
      </p>
      <UiButton
        type="submit"
        :disabled="!canSubmit"
        class="w-full"
      >
        {{ submitting ? '…' : t('onboarding.create.submit') }}
      </UiButton>
    </form>
  </UiSheet>
</template>
