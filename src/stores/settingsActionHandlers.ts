import { invoke } from '@tauri-apps/api/core'
import { useDevice } from '~/composables/useDevice'
import { useHuggingFace } from '~/composables/useHuggingFace'
import { useModels } from '~/composables/useModels'
import { usePreferences, type PrefScope } from '~/composables/usePreferences'
import { useProviders, type DelegateVendor } from '~/composables/useProviders'
import { useSttModels } from '~/composables/useSttModels'
import type { useWindowManagerStore } from '~/stores/windowManager'

type WmStore = ReturnType<typeof useWindowManagerStore>

/** Preference keys the settings components read (specs 002, 008, 009). */
const DEFAULT_MODEL_KEY = 'chat.default_model_id'
const STT_MODEL_KEY = 'voice.stt_model_id'
const AUTONOMY_KEY = 'chat.autonomy_mode'
const DENY_RULES_KEY = 'cli_delegate.deny_rules'

/**
 * Global handlers of the settings actions (spec 020-tab-navigation, T052,
 * `lib/actions/settingsActions.ts`). They hold the write logic the settings
 * components used to run themselves, so a setting changes the same way from
 * the UI, a shortcut or an agent, with or without the settings window open.
 */
export function registerSettingsActionHandlers(wm: WmStore): void {
  const { currentDeviceInfoAsync, updateDeviceAliasAsync } = useDevice()
  const { getPrefAsync, setPrefAsync, clearPrefAsync } = usePreferences()
  const models = useModels()
  const huggingFace = useHuggingFace()
  const providers = useProviders()
  const sttModels = useSttModels()
  const done = { done: true }
  const on = wm.registerGlobalActionHandler

  async function deviceScope(): Promise<PrefScope> {
    const device = await currentDeviceInfoAsync()
    return { kind: 'device', uuid: device.vaultDeviceUuid }
  }
  async function scopeOf(value: unknown): Promise<PrefScope> {
    return value === 'vault' ? { kind: 'vault' } : deviceScope()
  }

  on('settings.get', async () => {
    const device = await currentDeviceInfoAsync()
    const scope: PrefScope = { kind: 'device', uuid: device.vaultDeviceUuid }
    const [deviceDefault, vaultDefault, stt, autonomy, denyRules] =
      await Promise.all([
        getPrefAsync(scope, DEFAULT_MODEL_KEY),
        getPrefAsync({ kind: 'vault' }, DEFAULT_MODEL_KEY),
        getPrefAsync(scope, STT_MODEL_KEY),
        getPrefAsync(scope, AUTONOMY_KEY),
        getPrefAsync(scope, DENY_RULES_KEY),
      ])
    return {
      deviceAlias: device.alias,
      defaultModel: { device: deviceDefault, vault: vaultDefault },
      sttModel: stt,
      autonomyMode: autonomy,
      delegateDenyRules: denyRules ? (JSON.parse(denyRules) as unknown) : [],
    }
  })
  on('settings.models.list', async () => ({
    models: (await models.listInstalledAsync()).map((model) => ({
      modelId: model.id,
      name: model.name,
      sizeBytes: model.sizeBytes,
    })),
  }))
  on('settings.models.checkUpdates', async () => ({
    statuses: await huggingFace.checkUpdatesAsync(),
  }))

  on('settings.device.setAlias', async ({ input }) => {
    const alias = String(input.alias).trim()
    if (!alias) throw new Error('the device name must not be empty')
    await updateDeviceAliasAsync(alias)
    return done
  })
  on('settings.models.setDefault', async ({ input }) => {
    await setPrefAsync(
      await scopeOf(input.scope),
      DEFAULT_MODEL_KEY,
      String(input.modelId),
    )
    return done
  })
  on('settings.models.clearDefault', async ({ input }) => {
    await clearPrefAsync(await scopeOf(input.scope), DEFAULT_MODEL_KEY)
    return done
  })
  on('settings.models.setStt', async ({ input }) => {
    const installed = await sttModels.downloadFromCatalogAsync(
      String(input.catalogId),
    )
    await setPrefAsync(await deviceScope(), STT_MODEL_KEY, installed.id)
    await invoke('invalidate_stt_model_cache')
    return { modelId: installed.id }
  })
  on('settings.models.downloadCatalog', async ({ input }) => {
    await models.downloadFromCatalogAsync(String(input.entryId))
    return done
  })
  on('settings.models.downloadFromHf', async ({ input }) =>
    models.downloadFromHfAsync({
      repoId: String(input.repoId),
      filename: String(input.filename),
      revision: typeof input.revision === 'string' ? input.revision : undefined,
      name:
        typeof input.name === 'string' ? input.name : String(input.filename),
      tokenizerRepo:
        typeof input.tokenizerRepo === 'string'
          ? input.tokenizerRepo
          : undefined,
      contextWindow:
        typeof input.contextWindow === 'number'
          ? input.contextWindow
          : undefined,
      forceTooBig: input.forceTooBig === true,
    }),
  )
  on('settings.models.installUpdate', async ({ input }) => {
    await huggingFace.installUpdateAsync(String(input.modelId))
    return done
  })
  on('settings.models.delete', async ({ input }) => {
    await models.deleteAsync(String(input.modelId))
    return done
  })
  on('settings.delegate.refreshModels', async ({ input }) => {
    await providers.refreshModelsAsync(String(input.providerId))
    return done
  })
  on('settings.delegate.connectProvider', async ({ input }) => {
    const result = await providers.connectCliDelegateAsync({
      vendor: input.vendor as DelegateVendor,
      name: String(input.name),
    })
    return { status: result.status }
  })
  on('settings.delegate.submitCode', async ({ input }) => {
    await providers.submitCliDelegateCodeAsync({
      code: String(input.code),
      name: String(input.name),
    })
    return done
  })
  on('settings.autonomy.setMode', async ({ input }) => {
    await setPrefAsync(await deviceScope(), AUTONOMY_KEY, String(input.mode))
    return done
  })
  on('settings.delegate.setDenyRules', async ({ input }) => {
    await setPrefAsync(
      await deviceScope(),
      DENY_RULES_KEY,
      JSON.stringify(input.rules),
    )
    return done
  })
}
