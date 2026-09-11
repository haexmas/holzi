<script setup lang="ts">
import type { InstalledModel } from '~/composables/useModels'
import type { Provider, ProviderModel } from '~/composables/useProviders'

const { t } = useI18n()
const { getPrefAsync, setPrefAsync, clearPrefAsync } = usePreferences()
const { listInstalledAsync } = useModels()
const { listAsync: listProvidersAsync, listModelsAsync } = useProviders()

const props = defineProps<{
  deviceUuid: string
}>()

type ScopeKind = 'device' | 'vault'

const PREF_KEY = 'chat.default_model_id'

const installedModels = ref<InstalledModel[]>([])
const providerList = ref<Provider[]>([])
const providerModels = ref<Record<string, ProviderModel[]>>({})
const currentDeviceDefault = ref<string | null>(null)
const currentVaultDefault = ref<string | null>(null)

const selectedScope = ref<ScopeKind>('device')
const selectedModelId = ref<string>('')

const loading = ref(true)
const busy = ref(false)
const savedFlash = ref<'saved' | 'cleared' | null>(null)
const opError = ref<string | null>(null)
const loadError = ref<string | null>(null)

type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string, name: string }[]
}

const modelGroups = computed<ModelGroup[]>(() => {
  const groups: ModelGroup[] = []
  if (installedModels.value.length > 0) {
    groups.push({
      providerId: 'local',
      providerName: t('settings.default.modelGroup.local'),
      models: installedModels.value.map((m) => ({ id: m.id, name: m.name })),
    })
  }
  for (const p of providerList.value) {
    if (p.kind !== 'api_key') continue
    const list = providerModels.value[p.id] ?? []
    if (list.length === 0) continue
    groups.push({
      providerId: p.id,
      providerName: p.name,
      models: list.map((m) => ({ id: m.id, name: m.name })),
    })
  }
  return groups
})

const hasAnyModel = computed(() => modelGroups.value.some((g) => g.models.length > 0))

const currentForScope = computed(() =>
  selectedScope.value === 'device' ? currentDeviceDefault.value : currentVaultDefault.value,
)

/** Human-readable name for a stored model id, falling back to the raw id. */
function modelDisplayName(id: string): string {
  for (const g of modelGroups.value) {
    const hit = g.models.find((m) => m.id === id)
    if (hit) return hit.name
  }
  return id
}

async function reloadAsync() {
  loading.value = true
  loadError.value = null
  try {
    const [installed, providers] = await Promise.all([
      listInstalledAsync(),
      listProvidersAsync(),
    ])
    installedModels.value = installed
    providerList.value = providers
    const nextModels: Record<string, ProviderModel[]> = {}
    await Promise.all(
      providers
        .filter((p) => p.kind === 'api_key')
        .map(async (p) => {
          try {
            nextModels[p.id] = await listModelsAsync(p.id)
          }
          catch {
            nextModels[p.id] = []
          }
        }),
    )
    providerModels.value = nextModels

    const [deviceVal, vaultVal] = await Promise.all([
      getPrefAsync({ kind: 'device', uuid: props.deviceUuid }, PREF_KEY),
      getPrefAsync({ kind: 'vault' }, PREF_KEY),
    ])
    currentDeviceDefault.value = deviceVal
    currentVaultDefault.value = vaultVal

    // Seed the selector with the current value for the initial scope so
    // the operator sees what is stored and edits deliberately.
    const seed = selectedScope.value === 'device' ? deviceVal : vaultVal
    selectedModelId.value = seed ?? ''
  }
  catch (e) {
    loadError.value = e instanceof Error ? e.message : String(e)
  }
  finally {
    loading.value = false
  }
}

watch(selectedScope, (scope) => {
  const seed = scope === 'device' ? currentDeviceDefault.value : currentVaultDefault.value
  selectedModelId.value = seed ?? ''
  savedFlash.value = null
  opError.value = null
})

async function onSave() {
  if (!selectedModelId.value) return
  busy.value = true
  savedFlash.value = null
  opError.value = null
  const scope
    = selectedScope.value === 'device'
      ? { kind: 'device' as const, uuid: props.deviceUuid }
      : { kind: 'vault' as const }
  try {
    await setPrefAsync(scope, PREF_KEY, selectedModelId.value)
    if (selectedScope.value === 'device') {
      currentDeviceDefault.value = selectedModelId.value
    }
    else {
      currentVaultDefault.value = selectedModelId.value
    }
    savedFlash.value = 'saved'
  }
  catch (e) {
    opError.value = e instanceof Error ? e.message : String(e)
  }
  finally {
    busy.value = false
  }
}

async function onClear() {
  busy.value = true
  savedFlash.value = null
  opError.value = null
  const scope
    = selectedScope.value === 'device'
      ? { kind: 'device' as const, uuid: props.deviceUuid }
      : { kind: 'vault' as const }
  try {
    await clearPrefAsync(scope, PREF_KEY)
    if (selectedScope.value === 'device') {
      currentDeviceDefault.value = null
    }
    else {
      currentVaultDefault.value = null
    }
    selectedModelId.value = ''
    savedFlash.value = 'cleared'
  }
  catch (e) {
    opError.value = e instanceof Error ? e.message : String(e)
  }
  finally {
    busy.value = false
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-3">
    <h2 class="text-xl font-semibold">
      {{ t('settings.default.title') }}
    </h2>
    <p class="text-sm text-neutral-500">
      {{ t('settings.default.description') }}
    </p>

    <div v-if="loading" class="text-sm text-neutral-500">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-red-500" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <template v-if="!loading">
      <div class="flex flex-col gap-1 text-sm">
        <span>
          {{ t('settings.default.current.device') }}:
          <strong v-if="currentDeviceDefault">{{ modelDisplayName(currentDeviceDefault) }}</strong>
          <em v-else class="text-neutral-500">{{ t('settings.default.current.none') }}</em>
        </span>
        <span>
          {{ t('settings.default.current.vault') }}:
          <strong v-if="currentVaultDefault">{{ modelDisplayName(currentVaultDefault) }}</strong>
          <em v-else class="text-neutral-500">{{ t('settings.default.current.none') }}</em>
        </span>
      </div>

      <div v-if="!hasAnyModel" class="text-sm text-neutral-500">
        {{ t('settings.default.empty') }}
      </div>

      <template v-else>
        <fieldset class="flex flex-col gap-1">
          <legend class="text-sm font-medium">
            {{ t('settings.default.scope.label') }}
          </legend>
          <label class="flex items-center gap-2 text-sm">
            <input
              v-model="selectedScope"
              type="radio"
              value="device"
              :disabled="busy"
            >
            {{ t('settings.default.scope.device') }}
          </label>
          <label class="flex items-center gap-2 text-sm">
            <input
              v-model="selectedScope"
              type="radio"
              value="vault"
              :disabled="busy"
            >
            {{ t('settings.default.scope.vault') }}
          </label>
        </fieldset>

        <label class="flex flex-col gap-1">
          <span class="text-sm font-medium">{{ t('settings.default.modelLabel') }}</span>
          <select
            v-model="selectedModelId"
            class="border border-neutral-300 rounded-md p-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            :disabled="busy"
          >
            <option value="" disabled>
              {{ t('settings.default.modelPlaceholder') }}
            </option>
            <optgroup
              v-for="group in modelGroups"
              :key="group.providerId"
              :label="group.providerName"
            >
              <option
                v-for="m in group.models"
                :key="m.id"
                :value="m.id"
              >
                {{ m.name }}
              </option>
            </optgroup>
          </select>
        </label>

        <div class="flex items-center gap-3 flex-wrap">
          <UiButton
            type="button"
            :disabled="busy || !selectedModelId || selectedModelId === (currentForScope ?? '')"
            @click="onSave"
          >
            {{ t('settings.default.save') }}
          </UiButton>
          <UiButton
            v-if="currentForScope"
            type="button"
            variant="outline"
            :disabled="busy"
            @click="onClear"
          >
            {{ t('settings.default.clear') }}
          </UiButton>
          <span v-if="savedFlash === 'saved'" class="text-xs text-green-600" role="status">
            {{ t('settings.default.saved') }}
          </span>
          <span v-if="savedFlash === 'cleared'" class="text-xs text-green-600" role="status">
            {{ t('settings.default.cleared') }}
          </span>
          <span v-if="opError" class="text-xs text-red-500" role="alert">
            {{ t('errors.prefSaveFailed') }}: {{ opError }}
          </span>
        </div>
      </template>
    </template>
  </section>
</template>
