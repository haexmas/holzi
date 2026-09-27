// Color scheme (spec 023-settings-app, FR-013, FR-014, FR-024, research R8, data-model.md):
// light, dark or following the system, one value for the whole vault. Pure — `useColorScheme`
// applies it, `scripts/check-settings.ts` tests it.

export type ColorScheme = 'light' | 'dark' | 'system'

/** The vault preference key. */
export const COLOR_SCHEME_KEY = 'appearance.color_scheme'

/** Any other stored value counts as unset, which means `system`. */
export function parseColorScheme(value: unknown): ColorScheme | null {
  return value === 'light' || value === 'dark' || value === 'system'
    ? value
    : null
}

export function isDark(scheme: ColorScheme, systemDark: boolean): boolean {
  return scheme === 'dark' || (scheme === 'system' && systemDark)
}
