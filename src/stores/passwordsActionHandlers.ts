import { invoke } from '@tauri-apps/api/core'
import { projectSearchResult } from '~/lib/actions/passwordsActions'
import type { useWindowManagerStore } from '~/stores/windowManager'

type WmStore = ReturnType<typeof useWindowManagerStore>

/**
 * The global handler of the password manager action (spec 034, FR-027,
 * `lib/actions/passwordsActions.ts`): `passwords.items.search` calls the one backend command that
 * runs as the built-in agent. The answer is projected to the five fields of the result schema, so
 * even a wider backend answer could not leak a username, an address or a secret to the model.
 */
export function registerPasswordsActionHandlers(wm: WmStore): void {
  wm.registerGlobalActionHandler(
    'passwords.items.search',
    async ({ input }) => {
      const raw = await invoke<unknown>('passwords_agent_search', {
        args: {
          query: typeof input.query === 'string' ? input.query : undefined,
          tag: typeof input.tag === 'string' ? input.tag : undefined,
          limit: typeof input.limit === 'number' ? input.limit : undefined,
        },
      })
      return projectSearchResult(raw)
    },
  )
}
