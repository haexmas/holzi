// The colour fields of the appearance settings (spec 035-appearance-and-fields, data-model.md
// "Farbfelder"). Hue and chroma are OKLCH; the language key of each is `settings.appearance.preset.<id>`.

export type Preset = { id: string; h: number; c: number }

export const ACCENT_PRESETS: readonly Preset[] = [
  { id: 'teal', h: 180, c: 0.17 },
  { id: 'blue', h: 255, c: 0.17 },
  { id: 'violet', h: 300, c: 0.17 },
  { id: 'pink', h: 350, c: 0.17 },
  { id: 'red', h: 25, c: 0.17 },
  { id: 'orange', h: 55, c: 0.17 },
  { id: 'yellow', h: 95, c: 0.17 },
  { id: 'green', h: 150, c: 0.17 },
  { id: 'neutral', h: 0, c: 0 },
]

export const TINT_PRESETS: readonly Preset[] = [
  { id: 'neutral', h: 0, c: 0 },
  { id: 'warm', h: 60, c: 0.02 },
  { id: 'cool', h: 240, c: 0.02 },
  { id: 'green', h: 150, c: 0.02 },
  { id: 'violet', h: 300, c: 0.02 },
  { id: 'rose', h: 10, c: 0.02 },
]

export const presetKey = (id: string) => `settings.appearance.preset.${id}`
