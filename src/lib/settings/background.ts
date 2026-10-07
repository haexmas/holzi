// Workspace background (spec 042, FR-016–FR-018, research R6, data-model.md): one image for the
// whole vault, a WebP data URL. Pure — `useWorkspaceBackground` applies it,
// `scripts/check-settings.ts` tests it.

/** The vault preference key. */
export const BACKGROUND_KEY = 'appearance.background'

/** The longer edge of a stored background in pixels. */
export const BACKGROUND_MAX_EDGE = 2560

/** Only base64 after the prefix, so the value cannot leave the CSS `url("…")` it is shown in. */
const VALUE = /^data:image\/webp;base64,[A-Za-z0-9+/]+=*$/

/** A stored value is shown only as the WebP data URL the settings write; it also arrives by sync. */
export function isBackgroundValue(value: unknown): value is string {
  return typeof value === 'string' && VALUE.test(value)
}
