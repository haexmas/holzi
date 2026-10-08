// Scenarios that do not run on a platform (spec 043 FR-033b, data-model.md). `excluded` is final: the
// function does not exist there (FR-016) or works differently (FR-006), and a counter case checks the
// "not available" answer. `pending` runs there once the named stage of the plan is done; after stage 5
// none is left (`PENDING_ALLOWED` turns false). `pnpm check:e2e-exclusions` checks these rules.

export type Platform = 'android'

export type PlatformExclusion =
  | {
      scenario: string
      platform: Platform
      kind: 'excluded'
      /** Why, with the requirement (FR-006 or FR-016). */
      reason: string
      /** The scenario that checks the "not available" answer (or the different behaviour) instead. */
      counterCase: string
    }
  | {
      scenario: string
      platform: Platform
      kind: 'pending'
      /** The stage of the plan that makes it run (`2`, `3`, `1c`, …). */
      stage: string
      reason: string
    }

/** False from stage 5 on: every scenario then runs or is excluded for good (SC-007). */
export const PENDING_ALLOWED = true

/** The least share of the desktop scenarios that must run on Android (SC-007). */
export const MINIMUM_COVERAGE = 0.8

export const EXCLUSIONS: PlatformExclusion[] = [
  {
    scenario: 'extension-files',
    platform: 'android',
    kind: 'excluded',
    reason:
      'free file paths and folder watching do not exist on Android (FR-016)',
    counterCase: 'android-not-available',
  },
  {
    scenario: 'extension-dev-mode',
    platform: 'android',
    kind: 'excluded',
    reason:
      'loads an extension from a project folder, which needs a folder path (FR-016)',
    counterCase: 'android-not-available',
  },
  {
    scenario: 'relaunch-after-lock',
    platform: 'android',
    kind: 'excluded',
    reason:
      'closing the vault ends the app on Android; there is no relaunch (FR-006)',
    counterCase: 'lock-twice',
  },
  {
    scenario: 'passwords-organize',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason:
      'drops a selection on the breadcrumbs, which hide while a selection is active (spec 036); on Linux the drop lands while they fade out, on the emulator they are already gone. To be settled in spec 036 before stage 2',
  },
  {
    scenario: 'appearance-sync-two-devices',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'extension-two-devices',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'passwords-sync-two-devices',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'storage-two-devices',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-away-through-key-change',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-copy',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-identity',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-indirect',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-link',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-lock-during-sync',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-mutual-removal',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-own-devices-only',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-presence',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-relay-return',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-remove-device',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-servers-off',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-two-devices',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
  {
    scenario: 'sync-two-users',
    platform: 'android',
    kind: 'pending',
    stage: '2',
    reason: 'needs a group that mixes the Android device with Linux devices',
  },
]

/** The entries of `platform`, by scenario. */
export function exclusionsFor(
  platform: Platform,
  entries: PlatformExclusion[] = EXCLUSIONS,
): Map<string, PlatformExclusion> {
  return new Map(
    entries
      .filter((entry) => entry.platform === platform)
      .map((entry) => [entry.scenario, entry]),
  )
}
