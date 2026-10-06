// The CSS variables of an appearance for one colour scheme (spec 035-appearance-and-fields,
// contracts/token-map.md, research R4). Pure: the same function runs in the app and in
// `scripts/check-appearance*.ts`, which measures every pair of the token map for every colour field.
//
// Order: the tints give the surfaces and texts at the lightness the scheme has anyway; pairs that
// would be too faint lose chroma first, then the text moves in lightness; the accent then looks for the
// lightness nearest to its own at which its label and its role as a control are readable.
import {
  contrast,
  contrastRgb,
  solveLightness,
  CONTROL_MIN,
  TEXT_MIN,
} from './contrast.ts'
import {
  fitTint,
  formatOklch,
  mapToGamut,
  oklchToRgb,
  parseHex,
  rgbToOklch,
  type Oklch,
} from './oklch.ts'
import { ACCENT_PRESETS, TINT_PRESETS } from './presets.ts'
import type { Appearance, ColorChoice, Control } from './schema.ts'
import {
  CONTROL_PAIRS,
  DEFAULT_LIGHTNESS,
  SURFACES,
  TEXT_PAIRS,
  TOKEN_CONTROL,
  TOKEN_NAMES,
  type Scheme,
  type TintToken,
  type TokenName,
} from './tokens.ts'

export type AdjustmentReason = 'accent-contrast' | 'text-contrast'
export interface Adjustment {
  control: Control
  kind: 'lightness' | 'chroma'
  reason: AdjustmentReason
}
export interface Derived {
  tokens: Record<TokenName, string>
  adjustments: Adjustment[]
}

/** Highest chroma a tint may carry; more would turn a surface into a colour. */
const CHROMA_CAP = {
  window: 0.03,
  container: 0.03,
  component: 0.04,
  text: 0.04,
}

/** Chroma a tint keeps per unit of distance from white: light surfaces fade towards grey like
 * Tailwind's slate (50: L 0.984 / C 0.003, 300: L 0.869 / C 0.022), so they stay in gamut at their
 * own lightness instead of being pushed darker. */
const TAPER = 0.2

/** Safety margin over the limits, so rounding the values to three decimals cannot cross them. */
const MARGIN = 0.02

const WHITE: Oklch = { l: 0.985, c: 0, h: 0 }
const BLACK: Oklch = { l: 0.145, c: 0, h: 0 }
const ACCENT_START: Record<Scheme, number> = { light: 0.65, dark: 0.7 }

/** Resolves a tint's hue and capped chroma; an invalid choice falls back to a neutral tint. */
function tintOf(
  control: Exclude<Control, 'accent'>,
  choice: ColorChoice,
): { h: number; c: number } {
  const cap = CHROMA_CAP[control]
  if ('preset' in choice) {
    const preset =
      TINT_PRESETS.find((p) => p.id === choice.preset) ?? TINT_PRESETS[0]!
    return { h: preset.h, c: Math.min(preset.c, cap) }
  }
  const rgb = parseHex(choice.custom)
  if (!rgb) return { h: 0, c: 0 }
  const { h, c } = rgbToOklch(rgb)
  return { h, c: Math.min(c, cap) }
}

/** Resolves the initial accent, using scheme lightness for presets and a teal fallback. */
function accentStart(choice: ColorChoice, scheme: Scheme): Oklch {
  if ('preset' in choice) {
    const preset =
      ACCENT_PRESETS.find((p) => p.id === choice.preset) ?? ACCENT_PRESETS[0]!
    return { l: ACCENT_START[scheme], c: preset.c, h: preset.h }
  }
  const rgb = parseHex(choice.custom)
  return rgb ? rgbToOklch(rgb) : { l: ACCENT_START[scheme], c: 0.17, h: 180 }
}

/**
 * Derives CSS tokens for a resolved scheme without changing the stored appearance. Adjusts tints
 * and accent lightness for contrast and returns adjustment notices alongside the token strings.
 */
export function derive(appearance: Appearance, scheme: Scheme): Derived {
  const adjustments: Adjustment[] = []
  const note = (adjustment: Adjustment) => {
    const known = adjustments.some(
      (a) => a.control === adjustment.control && a.kind === adjustment.kind,
    )
    if (!known) adjustments.push(adjustment)
  }

  const tints = {
    window: tintOf('window', appearance.window),
    container: tintOf('container', appearance.container),
    component: tintOf('component', appearance.component),
    text: tintOf('text', appearance.text),
  }
  const scale: Record<keyof typeof tints, number> = {
    window: 1,
    container: 1,
    component: 1,
    text: 1,
  }
  const shift: Partial<Record<TintToken, number>> = {}

  const tintColor = (name: TintToken): Oklch => {
    const control = TOKEN_CONTROL[name] as keyof typeof tints
    const tint = tints[control]
    const l = DEFAULT_LIGHTNESS[scheme][name] + (shift[name] ?? 0)
    return fitTint({
      l,
      c: Math.min(tint.c * scale[control], TAPER * (1 - l)),
      h: tint.h,
    })
  }
  const isTint = (name: TokenName): name is TintToken =>
    TOKEN_CONTROL[name] !== 'accent'

  // Text pairs of tinted tokens: chroma first, then the lightness of the text.
  const textPairs = TEXT_PAIRS.filter(
    (pair): pair is readonly [TintToken, TintToken] =>
      isTint(pair[0]) && isTint(pair[1]),
  )
  for (let round = 0; round < 12; round += 1) {
    let changed = false
    for (const [fg, bg] of textPairs) {
      if (contrast(tintColor(fg), tintColor(bg)) >= TEXT_MIN + MARGIN) continue
      const controls = [
        ...new Set([TOKEN_CONTROL[fg], TOKEN_CONTROL[bg]]),
      ] as (keyof typeof tints)[]
      const reducible = controls.filter((control) => scale[control] > 0)
      if (reducible.length > 0) {
        for (const control of reducible) {
          scale[control] = Math.max(0, scale[control] - 0.25)
          note({ control, kind: 'chroma', reason: 'text-contrast' })
        }
      } else {
        shift[fg] = (shift[fg] ?? 0) + (scheme === 'light' ? -0.005 : 0.005)
        note({
          control: TOKEN_CONTROL[fg] as Control,
          kind: 'lightness',
          reason: 'text-contrast',
        })
      }
      changed = true
    }
    if (!changed) break
  }

  // The accent: the lightness nearest to its own at which a label on it and the accent as a control
  // against every surface are readable.
  const surfaceRgb = SURFACES.map((name) => oklchToRgb(tintColor(name)))
  const whiteRgb = oklchToRgb(WHITE)
  const blackRgb = oklchToRgb(BLACK)
  const start = accentStart(appearance.accent, scheme)
  const solved = solveLightness(start, (candidate) => {
    const rgb = oklchToRgb(candidate)
    return (
      Math.max(contrastRgb(rgb, whiteRgb), contrastRgb(rgb, blackRgb)) >=
        TEXT_MIN + MARGIN &&
      surfaceRgb.every(
        (surface) => contrastRgb(rgb, surface) >= CONTROL_MIN + MARGIN,
      )
    )
  })
  const accent = mapToGamut(solved ?? start)
  const accentRgb = oklchToRgb(accent)
  const label =
    contrastRgb(accentRgb, whiteRgb) >= TEXT_MIN + MARGIN ? WHITE : BLACK
  if (
    'custom' in appearance.accent &&
    solved !== null &&
    Math.abs(solved.l - start.l) > 0.0015
  ) {
    note({ control: 'accent', kind: 'lightness', reason: 'accent-contrast' })
  }

  const tokens = {} as Record<TokenName, string>
  for (const name of TOKEN_NAMES) {
    if (isTint(name)) tokens[name] = formatOklch(tintColor(name))
  }
  const accentText = formatOklch(accent)
  const labelText = formatOklch(label)
  tokens.primary = accentText
  tokens.ring = accentText
  tokens['sidebar-primary'] = accentText
  tokens['sidebar-ring'] = accentText
  tokens['primary-foreground'] = labelText
  tokens['sidebar-primary-foreground'] = labelText
  return { tokens, adjustments }
}

export { CONTROL_PAIRS, TEXT_PAIRS }
