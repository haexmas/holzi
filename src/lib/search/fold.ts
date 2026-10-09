// Text folding shared by the searches (passwords, settings, app names). Pure, no imports.

/** Case- and accent-insensitive, and a decomposed umlaut equals a composed one: "bank" finds
 * "Bänk", "gerat" finds "Gerät". */
export function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
}
