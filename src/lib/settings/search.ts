// Settings search (spec 023-settings-app, FR-023, research R13): finds locations by title,
// description and synonyms, and single settings by their label. Pure — the caller passes the
// translation, so `scripts/check-settings.ts` runs it without vue-i18n.
import {
  locationPath,
  SETTINGS_LOCATIONS,
  type SettingsLocation,
} from './registry.ts'
import { fold } from '../search/fold.ts'

export type SettingsSearchHit = {
  location: SettingsLocation
  /** Where the hit leads: the location's own path. */
  path: string
  /** The location's title, or the label of the single setting that matched. */
  label: string
  /** Titles from the category down to the hit's location, without the label itself. */
  trail: string[]
}

type Entry = SettingsSearchHit & { text: string; order: number }

function ancestors(location: SettingsLocation): SettingsLocation[] {
  const parent = SETTINGS_LOCATIONS.find(
    (candidate) => candidate.id === location.parent,
  )
  return parent ? [...ancestors(parent), parent] : []
}

function entries(translate: (key: string) => string): Entry[] {
  return SETTINGS_LOCATIONS.filter((location) => location.keywordsKey).flatMap(
    (location, order) => {
      const title = translate(location.titleKey)
      const trail = ancestors(location).map((a) => translate(a.titleKey))
      const own: Entry = {
        location,
        path: locationPath(location),
        label: title,
        trail,
        order,
        text: [
          title,
          location.descriptionKey ? translate(location.descriptionKey) : '',
          translate(location.keywordsKey!),
        ].join(' '),
      }
      const settings = (location.settingKeys ?? []).map((key) => ({
        location,
        path: locationPath(location),
        label: translate(key),
        trail: [...trail, title],
        order,
        text: translate(key),
      }))
      return [own, ...settings]
    },
  )
}

/** 0 = the label starts with the query, 1 = a word of the label does, 2 = the label contains
 * it, 3 = only description or synonyms match. */
function rank(label: string, query: string): number {
  if (label.startsWith(query)) return 0
  if (label.split(/\s+|-/).some((word) => word.startsWith(query))) return 1
  return label.includes(query) ? 2 : 3
}

/** Every word of `query` must occur; best matches first, then in registry order. An empty query
 * finds nothing. */
export function searchSettings(
  query: string,
  translate: (key: string) => string,
): SettingsSearchHit[] {
  const folded = fold(query.trim())
  const words = folded.split(/\s+/).filter(Boolean)
  if (words.length === 0) return []
  return entries(translate)
    .filter((entry) => {
      const text = fold(`${entry.label} ${entry.text}`)
      return words.every((word) => text.includes(word))
    })
    .map((entry) => ({ entry, score: rank(fold(entry.label), folded) }))
    .sort((a, b) => a.score - b.score || a.entry.order - b.entry.order)
    .map(({ entry: { location, path, label, trail } }) => ({
      location,
      path,
      label,
      trail,
    }))
}
