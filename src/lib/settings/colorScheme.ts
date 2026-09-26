// Color scheme (spec 023-settings-app, FR-013, FR-014, research R8, data-model.md): light, dark or
// following the system, this device's value before the vault's. Pure — `useColorScheme` applies
// it, `scripts/check-settings.ts` tests it.

export type ColorScheme = 'light' | 'dark' | 'system'

export type ColorSchemeState = {
  device: ColorScheme | null
  vault: ColorScheme | null
  effective: ColorScheme
}

/** The preference key for both scopes. */
export const COLOR_SCHEME_KEY = 'appearance.color_scheme'

/** Any other stored value counts as unset. */
export function parseColorScheme(value: unknown): ColorScheme | null {
  return value === 'light' || value === 'dark' || value === 'system'
    ? value
    : null
}

export function effectiveColorScheme(values: {
  device: ColorScheme | null
  vault: ColorScheme | null
}): ColorScheme {
  return values.device ?? values.vault ?? 'system'
}

export function colorSchemeState(
  device: ColorScheme | null,
  vault: ColorScheme | null,
): ColorSchemeState {
  return { device, vault, effective: effectiveColorScheme({ device, vault }) }
}

export function isDark(scheme: ColorScheme, systemDark: boolean): boolean {
  return scheme === 'dark' || (scheme === 'system' && systemDark)
}

/** The action result: unset values are left out, the action schema has no `null` (contracts §3). */
export function toColorSchemeResult(state: ColorSchemeState): {
  effective: ColorScheme
  device?: ColorScheme
  vault?: ColorScheme
} {
  return {
    effective: state.effective,
    ...(state.device ? { device: state.device } : {}),
    ...(state.vault ? { vault: state.vault } : {}),
  }
}
