<script setup lang="ts">
interface Props {
  open: boolean
  name: string | null
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  unlocked: [name: string]
}>()

const { t } = useI18n()
const { openAsync } = useInstance()

const passphrase = ref('')
const submitting = ref(false)
const error = ref<string | null>(null)

function reset() {
  passphrase.value = ''
  error.value = null
}

watch(() => props.open, (isOpen) => {
  if (!isOpen)
    reset()
})

const canSubmit = computed(() =>
  passphrase.value.length > 0 && !submitting.value && props.name != null,
)

async function onSubmit() {
  if (!canSubmit.value || props.name == null)
    return
  submitting.value = true
  error.value = null
  try {
    const info = await openAsync({
      name: props.name,
      passphrase: passphrase.value,
    })
    emit('unlocked', info.name)
    emit('update:open', false)
  }
  catch {
    // Contract FR-021: NotFound and WrongPassphrase MUST both surface
    // as the same generic message on the frontend. Typed discriminator
    // stays in logs only.
    error.value = t('errors.openFailed')
  }
  finally {
    submitting.value = false
  }
}
</script>

<template>
  <UiDrawerModal
    :open="props.open"
    :title="t('onboarding.unlock.title')"
    :description="props.name ?? undefined"
    @update:open="$emit('update:open', $event)"
  >
    <template #content>
      <form
        id="unlock-form"
        class="space-y-4 px-6 py-2"
        @submit.prevent="onSubmit"
      >
        <div class="space-y-1.5">
          <ShadcnLabel for="unlock-passphrase">
            {{ t('onboarding.unlock.passphrase') }}
          </ShadcnLabel>
          <UiInputPassword
            id="unlock-passphrase"
            v-model="passphrase"
            autofocus
          />
        </div>
        <p v-if="error" class="text-sm text-destructive" role="alert">
          {{ error }}
        </p>
      </form>
    </template>
    <template #footer>
      <UiButton
        form="unlock-form"
        type="submit"
        :disabled="!canSubmit"
        :loading="submitting"
        class="w-full"
      >
        {{ t('onboarding.unlock.submit') }}
      </UiButton>
    </template>
  </UiDrawerModal>
</template>
