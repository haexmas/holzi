// WCAG 2 contrast and a search for the lightness that reaches it (spec 035-appearance-and-fields,
// research R3 and R4). Pure.
import { oklchToRgb, type Oklch, type Rgb } from './oklch.ts'

/** Minimum contrast of text (WCAG 1.4.3) and of controls such as a focus ring (WCAG 1.4.11). */
export const TEXT_MIN = 4.5
export const CONTROL_MIN = 3

/** WCAG relative luminance of gamma-encoded sRGB channels in 0..1. */
export function luminance([red, green, blue]: Rgb): number {
  const linear = (value: number) =>
    value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

/** WCAG contrast ratio of two sRGB colours, from 1 (identical luminance) to 21 (black/white). */
export function contrastRgb(a: Rgb, b: Rgb): number {
  const first = luminance(a)
  const second = luminance(b)
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05)
}

/** Contrast of two OKLCH colours as painted (out-of-gamut chroma is mapped first). */
export function contrast(a: Oklch, b: Oklch): number {
  return contrastRgb(oklchToRgb(a), oklchToRgb(b))
}

/**
 * The colour nearest in lightness to `start` (same chroma and hue) that `accept` takes, found by
 * stepping outwards, darker candidate first at equal distance; null when none does.
 * ponytail: a fixed step of 0.002 (ceiling: that resolution; upgrade path: bisect inside the first
 * accepted step) — it keeps the result a pure function of its inputs and fast enough per change.
 */
export function solveLightness(
  start: Oklch,
  accept: (color: Oklch) => boolean,
  step = 0.002,
): Oklch | null {
  const lowest = 0.05
  const highest = 0.98
  const limit = Math.max(start.l - lowest, highest - start.l)
  for (let distance = 0; distance <= limit + 1e-9; distance += step) {
    for (const l of [start.l - distance, start.l + distance]) {
      if (l < lowest || l > highest) continue
      const candidate = { ...start, l: Number(l.toFixed(4)) }
      if (accept(candidate)) return candidate
    }
  }
  return null
}
