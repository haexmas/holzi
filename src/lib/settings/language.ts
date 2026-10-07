// Interface language (spec 042, FR-006–FR-010, research R2/R3, data-model.md): German or English,
// one value for the whole vault. Before a vault is open `@nuxtjs/i18n` takes the system's. Pure —
// `useLanguage` applies it, `scripts/check-settings.ts` tests it.

export type Language = 'de' | 'en'

/** The vault preference key. */
export const LANGUAGE_KEY = 'general.language'

export const LANGUAGES: readonly Language[] = ['de', 'en']

/** Any other stored value counts as unset: the vault then takes the active language. */
export function parseLanguage(value: unknown): Language | null {
  return value === 'de' || value === 'en' ? value : null
}

/** Each language in its own words, so it is found whatever language is shown. */
export const LANGUAGE_NAMES: Readonly<Record<Language, string>> = {
  de: 'Deutsch',
  en: 'English',
}
