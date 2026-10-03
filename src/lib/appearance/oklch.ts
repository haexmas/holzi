// OKLCH <-> sRGB for the appearance settings (spec 035-appearance-and-fields, research R3). Pure: the
// app and `scripts/check-appearance.ts` run the same maths, so the check measures what the user sees.
// Matrices after Björn Ottosson's OKLab reference.

export type Oklch = { l: number; c: number; h: number }
/** Gamma-encoded sRGB, each channel 0..1. */
export type Rgb = [number, number, number]

const toDegrees = (radians: number) => (radians * 180) / Math.PI
const toRadians = (degrees: number) => (degrees * Math.PI) / 180

function srgbToLinear(value: number): number {
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
}

function linearToSrgb(value: number): number {
  return value <= 0.0031308 ? 12.92 * value : 1.055 * value ** (1 / 2.4) - 0.055
}

/** Linear sRGB of an OKLCH colour, not clamped (a channel outside 0..1 means out of gamut). */
function oklchToLinear({ l, c, h }: Oklch): [number, number, number] {
  const a = c * Math.cos(toRadians(h))
  const b = c * Math.sin(toRadians(h))
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  return [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ]
}

const EPSILON = 1e-6

/** Whether the colour is inside the sRGB gamut. */
export function inGamut(color: Oklch): boolean {
  return oklchToLinear(color).every(
    (channel) => channel >= -EPSILON && channel <= 1 + EPSILON,
  )
}

/**
 * The colour with the chroma lowered until it fits into sRGB, lightness and hue kept. Browsers map
 * out-of-gamut `oklch()` values their own way; deriving the mapped value here and writing that value
 * means what is measured is what is painted.
 */
export function mapToGamut(color: Oklch): Oklch {
  if (inGamut(color)) return color
  let low = 0
  let high = color.c
  for (let i = 0; i < 24; i += 1) {
    const middle = (low + high) / 2
    if (inGamut({ ...color, c: middle })) low = middle
    else high = middle
  }
  return { ...color, c: low }
}

/**
 * The colour with its full chroma if a slightly lower lightness (at most `maxShift`) lets it fit,
 * else the chroma-limited colour. A surface at lightness 1 (white) cannot carry a tint; a tint on it
 * moves it just far down to show.
 */
export function fitTint(color: Oklch, maxShift = 0.03): Oklch {
  if (inGamut(color) || color.c === 0) return color
  for (let shift = 0.0025; shift <= maxShift + EPSILON; shift += 0.0025) {
    const moved = { ...color, l: color.l - shift }
    if (inGamut(moved)) return moved
  }
  return mapToGamut(color)
}

export function oklchToRgb(color: Oklch): Rgb {
  const mapped = mapToGamut(color)
  const linear = oklchToLinear(mapped)
  return linear.map((channel) =>
    linearToSrgb(Math.min(1, Math.max(0, channel))),
  ) as Rgb
}

export function rgbToOklch([red, green, blue]: Rgb): Oklch {
  const r = srgbToLinear(red)
  const g = srgbToLinear(green)
  const b = srgbToLinear(blue)
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  const lightness = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  const chroma = Math.hypot(a, bb)
  const hue = chroma < 1e-4 ? 0 : (toDegrees(Math.atan2(bb, a)) + 360) % 360
  return { l: lightness, c: chroma, h: hue }
}

const HEX = /^#[0-9a-fA-F]{6}$/

export function isHexColor(value: unknown): value is string {
  return typeof value === 'string' && HEX.test(value)
}

export function parseHex(hex: string): Rgb | null {
  if (!isHexColor(hex)) return null
  return [1, 3, 5].map(
    (start) => parseInt(hex.slice(start, start + 2), 16) / 255,
  ) as Rgb
}

export function toHex(rgb: Rgb): string {
  return `#${rgb
    .map((channel) =>
      Math.round(Math.min(1, Math.max(0, channel)) * 255)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`
}

const trim = (value: number) => Number(value.toFixed(3)).toString()

/** `oklch(L C H)` with three decimals; the hue of a grey is 0. */
export function formatOklch({ l, c, h }: Oklch): string {
  const chroma = Number(c.toFixed(3))
  return `oklch(${trim(l)} ${trim(chroma)} ${chroma === 0 ? '0' : trim(h)})`
}

const OKLCH = /^oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*\)$/

export function parseOklch(text: string): Oklch | null {
  const match = OKLCH.exec(text.trim())
  if (!match) return null
  return { l: Number(match[1]), c: Number(match[2]), h: Number(match[3]) }
}
