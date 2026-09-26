<script setup lang="ts">
/**
 * Default chat model (spec 002 US4), for this device and for the whole vault; the device value
 * wins. Two selects saved on selection (spec 023 FR-021): "Wie alle Geräte" and "Keins" are the
 * "not set" options and clear the stored value.
 */
import type { InstalledModel } from '~/composables/useModels'
import type { Provider, ProviderModel } from '~/composables/useProviders'
import { useSettingsDevice } from '~/components/settings/deviceContext'

type ScopeKind = 'device' | 'vault'

const { t } = useI18n()
const { errString } = useErrorString()
const { getPrefAsync } = usePreferences()
const setDefault = useActionOrThrow('settings.models.setDefault')
const clearDefault = useActionOrThrow('settings.models.clearDefault')
const { listInstalledAsync } = useModels()
const { listAsync: listProvidersAsync, listModelsAsync } = useProviders()
const device = useSettingsDevice()

const PREF_KEY = 'chat.default_model_id'

const installedModels = ref<InstalledModel[]>([])
const providerList = ref<Provider[]>([])
const providerModels = ref<Record<string, ProviderModel[]>>({})
const stored = ref<Record<ScopeKind, string | null>>({
  device: null,
  vault: null,
})

const loading = ref(true)
const busy = ref(false)
const savedFlash = ref(false)
const opError = ref<string | null>(null)
const loadError = ref<string | null>(null)
const modelListError = ref(false)

type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string }[]
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

const hasAnyModel = computed(() =>
  modelGroups.value.some((g) => g.models.length > 0),
)

/** A stored id that is no longer offered (a deleted model) stays visible and selected. */
function isOffered(id: string | null): boolean {
  return (
    id === null ||
    modelGroups.value.some((g) => g.models.some((m) => m.id === id))
  )
}

async function reloadAsync() {
  loading.value = true
  loadError.value = null
  modelListError.value = false
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
          } catch {
            modelListError.value = true
          }
        }),
    )
    providerModels.value = nextModels

    const uuid = device.info.value.vaultDeviceUuid
    const [deviceVal, vaultVal] = await Promise.all([
      getPrefAsync({ kind: 'device', uuid }, PREF_KEY),
      getPrefAsync({ kind: 'vault' }, PREF_KEY),
    ])
    stored.value = { device: deviceVal, vault: vaultVal }
  } catch (e) {
    loadError.value = errString(e)
  } finally {
    loading.value = false
  }
}

async function chooseAsync(scope: ScopeKind, event: Event) {
  const select = event.target as HTMLSelectElement
  const modelId = select.value || null
  busy.value = true
  savedFlash.value = false
  opError.value = null
  try {
    if (modelId) await setDefault({ modelId, scope })
    else await clearDefault({ scope })
    stored.value = { ...stored.value, [scope]: modelId }
    savedFlash.value = true
  } catch (e) {
    opError.value = errString(e)
    select.value = stored.value[scope] ?? ''
  } finally {
    busy.value = false
  }
}

onMounted(reloadAsync)
</script>

<template>
  <section class="flex flex-col gap-3">
    <div v-if="loading" class="text-sm text-muted-foreground">
      {{ t('onboarding.wizard.loadingDeviceInfo') }}
    </div>

    <p v-if="loadError" class="text-sm text-destructive" role="alert">
      {{ t('errors.prefLoadFailed') }}: {{ loadError }}
    </p>

    <template v-if="!loading && !loadError">
      <p v-if="modelListError" class="text-sm text-destructive" role="alert">
        {{ t('errors.modelListFailed') }}
      </p>
      <SettingsGroup
        v-if="!hasAnyModel && !modelListError"
        :label="t('settings.default.empty')"
        data-testid="settings-default-empty"
      >
        <SettingsRow
          to="/models/download"
          icon="lucide:download"
          :title="t('settings.locations.models.download.title')"
          :description="t('settings.locations.models.download.description')"
        />
        <SettingsRow
          to="/agents/providers"
          icon="lucide:plug"
          :title="t('settings.locations.agents.providers.title')"
          :description="t('settings.locations.agents.providers.description')"
        />
      </SettingsGroup>

      <SettingsGroup>
        <SettingsRow
          v-for="scope in ['device', 'vault'] as const"
          :key="scope"
          :title="t(`settings.default.${scope}Label`)"
          :label-for="`settings-default-${scope}-select`"
        >
          <select
            :id="`settings-default-${scope}-select`"
            class="h-9 w-64 max-w-full rounded-md border border-input bg-background px-2 text-sm focus:ring-2 focus:ring-ring focus:outline-none"
            :value="stored[scope] ?? ''"
            :disabled="busy"
            :data-testid="`settings-default-${scope}`"
            @change="chooseAsync(scope, $event)"
          >
            <option value="">
              {{
                t(
                  scope === 'device'
                    ? 'settings.default.followVault'
                    : 'settings.default.none',
                )
              }}
            </option>
            <option
              v-if="!isOffered(stored[scope])"
              :value="stored[scope] ?? ''"
            >
              {{ t('settings.default.unknownModel', { id: stored[scope] }) }}
            </option>
            <optgroup
              v-for="group in modelGroups"
              :key="group.providerId"
              :label="group.providerName"
            >
              <option v-for="m in group.models" :key="m.id" :value="m.id">
                {{ m.name }}
              </option>
            </optgroup>
          </select>
        </SettingsRow>
      </SettingsGroup>

      <span v-if="savedFlash" class="text-xs text-success" role="status">
        {{ t('settings.default.saved') }}
      </span>
      <span v-if="opError" class="text-xs text-destructive" role="alert">
        {{ t('errors.prefSaveFailed') }}: {{ opError }}
      </span>
    </template>
  </section>
</template>
