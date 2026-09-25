import { usePreferences } from '~/composables/usePreferences'
import type { useWindowManagerStore } from '~/stores/windowManager'

type WmStore = ReturnType<typeof useWindowManagerStore>

/** The vault-scoped preference `VoiceInputControl.vue` reads on mount (spec 008). */
export const VOICE_AUTO_SEND_PREF_KEY = 'voice.auto_send'

/**
 * Global handlers of the chat's model, reasoning and voice actions (spec
 * 020-tab-navigation, T050, `CHAT_MODEL_ACTIONS` in
 * `lib/actions/chatActions.ts`). They act on the global models store and
 * preferences, so they work without the chat being open.
 */
export function registerChatActionHandlers(wm: WmStore): void {
  // The models store calls `useI18n()` in its setup, which only works inside a component setup:
  // resolve it per call (the workspace page has created it by then), never at registration.
  const models = () => useModelsStore()
  const { setPrefAsync } = usePreferences()
  const done = { done: true }
  const on = wm.registerGlobalActionHandler

  on('chat.model.select', async ({ input }) => {
    await models().loadModel(String(input.modelId))
    return done
  })
  on('chat.reasoning.set', async ({ input }) => {
    await models().updateEffortLevel(
      typeof input.level === 'string' ? input.level : null,
    )
    return done
  })
  on('chat.model.retryLoad', async () => {
    await models().retryModelLoad()
    return done
  })
  on('chat.model.downloadRecommended', async ({ input }) => {
    const entry = models().catalogEntries.find((e) => e.id === input.entryId)
    if (!entry) throw new Error(`no catalog entry ${String(input.entryId)}`)
    await models().downloadCatalogEntry(entry)
    return done
  })
  on('chat.modelIntegrity.decide', async ({ input }) => {
    if (input.decision === 'loadUntrusted')
      await models().onIntegrityLoadUntrusted()
    else if (input.decision === 'repairSource')
      await models().onIntegrityRepairSource()
    else await models().onIntegrityChooseOther()
    return done
  })
  on('chat.voice.setAutoSend', async ({ input }) => {
    await setPrefAsync(
      { kind: 'vault' },
      VOICE_AUTO_SEND_PREF_KEY,
      String(input.enabled === true),
    )
    return done
  })
}
