import { computed } from 'vue'
import { usePreferences } from '~/composables/usePreferences'
import {
  LANGUAGE_KEY,
  LANGUAGE_NAMES,
  LANGUAGES,
  parseLanguage,
  type Language,
} from '~/lib/settings/language'

/** The choices of every language select, each language in its own words. */
const options = LANGUAGES.map((value) => ({
  value,
  label: LANGUAGE_NAMES[value],
}))

/**
 * The interface language (spec 042, FR-006–FR-010, research R3): before a vault is open the system's
 * (`@nuxtjs/i18n`, nothing stored), afterwards the vault's, one value for all its devices. A vault
 * without one takes the language active when it opens. Call it inside a setup or plugin: it holds
 * the app's `$i18n`.
 */
export function useLanguage() {
  const { getPrefAsync, setPrefAsync } = usePreferences()
  const i18n = useNuxtApp().$i18n

  const language = computed(() => parseLanguage(i18n.locale.value) ?? 'en')

  async function applyAsync(next: Language): Promise<void> {
    if (i18n.locale.value !== next) await i18n.setLocale(next)
  }

  /** The start page's choice (FR-007): switches without storing anything, so it lasts until the
   * app restarts. An unknown value changes nothing. */
  async function showAsync(value: string): Promise<void> {
    const next = parseLanguage(value)
    if (next) await applyAsync(next)
  }

  /** After unlock or creation (FR-009): the stored language wins, a missing one is stored. */
  async function loadAsync(): Promise<void> {
    const stored = parseLanguage(
      await getPrefAsync({ kind: 'vault' }, LANGUAGE_KEY),
    )
    if (stored) await applyAsync(stored)
    else await setPrefAsync({ kind: 'vault' }, LANGUAGE_KEY, language.value)
  }

  /** After a synced change (FR-010); an unset or unknown value keeps what is shown. */
  async function refreshAsync(): Promise<void> {
    const stored = parseLanguage(
      await getPrefAsync({ kind: 'vault' }, LANGUAGE_KEY),
    )
    if (stored) await applyAsync(stored)
  }

  /** Writes the vault's value and switches at once. */
  async function setAsync(next: Language): Promise<Language> {
    await setPrefAsync({ kind: 'vault' }, LANGUAGE_KEY, next)
    await applyAsync(next)
    return next
  }

  return { language, options, showAsync, loadAsync, refreshAsync, setAsync }
}
