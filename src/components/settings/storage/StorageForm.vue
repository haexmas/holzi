<script setup lang="ts">
/**
 * A storage on a connection (spec 038, US1, T028, FR-002, FR-004): its name and bucket. A new
 * storage or a new bucket is tested before it is saved; a new name is not. "Jetzt testen" runs the
 * test again and remembers its outcome on this device; a test object holzi could not delete is
 * named (SC-004). The credentials come from the connection, so none are typed here.
 */
import { invoke } from '@tauri-apps/api/core'
import type { StorageInput } from '@bindings/StorageInput'
import type { TestResult } from '@bindings/TestResult'

const { t } = useI18n()
const router = useTabRouter()
const fieldLabels = useFieldLabels()
const storageError = useStorageError()
const { overview, connection } = useStorageOverview()

const connectionId = computed(() => router.route.params.connectionId ?? '')
const storageId = computed(() => router.route.params.storageId ?? 'new')
const isNew = computed(() => storageId.value === 'new')
const parent = computed(() => connection(connectionId.value))
const existing = computed(() =>
  isNew.value
    ? undefined
    : overview.value?.storages.find((s) => s.id === storageId.value),
)
const missing = computed(
  () =>
    overview.value !== null &&
    (!parent.value || (!isNew.value && !existing.value)),
)

const draft = reactive({ name: '', bucket: '' })
const busy = ref(false)
const failure = ref<string | null>(null)
const tested = ref<string | null>(null)

let filled = false
watchEffect(() => {
  const stored = existing.value
  if (filled || !stored) return
  filled = true
  draft.name = stored.name
  draft.bucket = stored.bucket
})

async function saveAsync() {
  if (busy.value) return
  busy.value = true
  failure.value = null
  const input: StorageInput = {
    id: isNew.value ? undefined : storageId.value,
    connectionId: connectionId.value,
    name: draft.name.trim() || draft.bucket.trim(),
    bucket: draft.bucket.trim(),
  }
  try {
    await invoke('storage_save', { input })
    router.back()
  } catch (error) {
    failure.value = storageError(error)
  } finally {
    busy.value = false
  }
}

async function testAsync() {
  if (busy.value || isNew.value) return
  busy.value = true
  failure.value = null
  tested.value = null
  try {
    const result = await invoke<TestResult>('storage_test', {
      id: storageId.value,
    })
    const text = t(`settings.storage.outcome.${result.outcome}`)
    const message = result.leftoverKey
      ? `${text}. ${t('settings.storage.leftover', { key: result.leftoverKey })}`
      : text
    if (result.outcome === 'passed') tested.value = message
    else failure.value = message
  } catch (error) {
    failure.value = storageError(error)
  } finally {
    busy.value = false
  }
}

/** Enter in a field saves. No native form: haex-ui's eye button of a password field has no
 * `type="button"`, so it would be the form's default button and catch the Enter. */
function onEnter(event: KeyboardEvent) {
  if ((event.target as HTMLElement | null)?.tagName !== 'INPUT') return
  event.preventDefault()
  void saveAsync()
}
</script>

<template>
  <section class="flex flex-col gap-3" data-testid="storage-form">
    <p v-if="missing" class="px-1 text-sm text-muted-foreground" role="status">
      {{ t('settings.storage.notFound') }}
    </p>

    <div v-else class="flex flex-col gap-3" @keydown.enter="onEnter">
      <SettingsGroup :label="parent?.providerName">
        <li class="px-4 py-3">
          <UiInput
            id="storage-name"
            v-model="draft.name"
            :label="t('settings.storage.fields.storageName')"
            :placeholder="draft.bucket"
            :labels="fieldLabels.input.value"
            label-bg="var(--muted)"
            maxlength="80"
            data-testid="storage-name"
          />
        </li>
        <li class="px-4 py-3">
          <UiInput
            id="storage-bucket"
            v-model="draft.bucket"
            :label="t('settings.storage.fields.bucket')"
            :labels="fieldLabels.input.value"
            label-bg="var(--muted)"
            autocomplete="off"
            data-testid="storage-bucket"
          />
        </li>
        <li
          v-if="parent && parent.credentials !== 'present'"
          class="px-4 py-3 text-sm text-destructive"
          role="note"
        >
          {{
            t(`settings.storage.credentialsUnavailable.${parent.credentials}`)
          }}
        </li>
      </SettingsGroup>

      <p
        v-if="tested"
        class="px-1 text-sm text-muted-foreground"
        role="status"
        data-testid="storage-test-passed"
      >
        {{ tested }}
      </p>
      <p
        v-if="failure"
        class="px-1 text-sm text-destructive"
        role="alert"
        data-testid="storage-failure"
      >
        {{ failure }}
      </p>
      <div class="flex flex-wrap gap-2">
        <UiButton
          type="button"
          :loading="busy"
          data-testid="storage-save"
          @click="saveAsync"
        >
          {{
            isNew || draft.bucket.trim() !== existing?.bucket
              ? t('settings.storage.save')
              : t('settings.storage.saveName')
          }}
        </UiButton>
        <template v-if="!isNew">
          <UiButton
            type="button"
            variant="outline"
            :disabled="busy"
            data-testid="storage-test"
            @click="testAsync"
          >
            {{ t('settings.storage.test') }}
          </UiButton>
          <UiButton
            type="button"
            variant="outline"
            :disabled="busy"
            data-testid="storage-remove"
            @click="router.push(`/storage/remove/storage/${storageId}`)"
          >
            {{ t('settings.storage.remove') }}
          </UiButton>
        </template>
      </div>
    </div>
  </section>
</template>
