/**
 * Applies the system's color scheme at start, before any vault is unlocked (spec
 * 023-settings-app, FR-014, research R8); `pages/workspace/[instance].vue` loads the vault's
 * values once it is open.
 */
export default defineNuxtPlugin(() => {
  useColorScheme().startSystem()
})
