// Run with `node --test scripts/check-appearance*.ts`. Spec 035-appearance-and-fields, FR-016, FR-017,
// FR-022, SC-004: every colour field and the extreme values of a custom colour, in both colour
// schemes, against every pair of contracts/token-map.md — measured on the token strings the app
// writes, so rounding is part of the measurement.
import assert from 'node:assert/strict'
import test from 'node:test'
import {
  contrast,
  CONTROL_MIN,
  TEXT_MIN,
} from '../src/lib/appearance/contrast.ts'
import { derive } from '../src/lib/appearance/derive.ts'
import { oklchToRgb, parseOklch, toHex } from '../src/lib/appearance/oklch.ts'
import { ACCENT_PRESETS, TINT_PRESETS } from '../src/lib/appearance/presets.ts'
import {
  DEFAULT_APPEARANCE,
  type Appearance,
  type ColorChoice,
  type Control,
} from '../src/lib/appearance/schema.ts'
import {
  CONTROL_PAIRS,
  TEXT_PAIRS,
  type Scheme,
  type TokenName,
} from '../src/lib/appearance/tokens.ts'

const SCHEMES: Scheme[] = ['light', 'dark']

/** Hex values at the corners of what a user can pick: saturated hues every 15°, greys, black, white. */
function extremeHexes(): string[] {
  const hues = Array.from({ length: 24 }, (_, i) =>
    toHex(oklchToRgb({ l: 0.65, c: 0.4, h: i * 15 })),
  )
  return [
    ...hues,
    '#000000',
    '#ffffff',
    '#808080',
    '#ff0000',
    '#ffff00',
    '#00ffff',
    '#ff00ff',
    '#ffffaa',
  ]
}

function choicesFor(control: Control): ColorChoice[] {
  const presets = control === 'accent' ? ACCENT_PRESETS : TINT_PRESETS
  return [
    ...presets.map((preset) => ({ preset: preset.id })),
    ...extremeHexes().map((custom) => ({ custom })),
  ]
}

function measure(appearance: Appearance, scheme: Scheme, label: string) {
  const { tokens } = derive(appearance, scheme)
  const color = (name: TokenName) => {
    const parsed = parseOklch(tokens[name])
    assert.ok(parsed, `${label} ${scheme}: --${name} is ${tokens[name]}`)
    return parsed
  }
  const failures: string[] = []
  for (const [pairs, minimum] of [
    [TEXT_PAIRS, TEXT_MIN],
    [CONTROL_PAIRS, CONTROL_MIN],
  ] as const) {
    for (const [fg, bg] of pairs) {
      const ratio = contrast(color(fg), color(bg))
      if (ratio < minimum) {
        failures.push(
          `${label} ${scheme}: --${fg} on --${bg} is ${ratio.toFixed(2)}, below ${minimum}`,
        )
      }
    }
  }
  return failures
}

test('the defaults reach the limits in both schemes', () => {
  const failures = SCHEMES.flatMap((scheme) =>
    measure(DEFAULT_APPEARANCE, scheme, 'default'),
  )
  assert.deepEqual(failures, [])
})

for (const control of ['accent', 'window', 'container'] as const) {
  test(`every ${control} colour field and extreme custom colour reaches the limits`, () => {
    const failures: string[] = []
    for (const choice of choicesFor(control)) {
      for (const scheme of SCHEMES) {
        failures.push(
          ...measure(
            { ...DEFAULT_APPEARANCE, [control]: choice },
            scheme,
            `${control} ${JSON.stringify(choice)}`,
          ),
        )
      }
    }
    assert.deepEqual(failures, [])
  })
}

test('an accent preset reports no adjustment; a custom one reports its lightness change', () => {
  for (const preset of ACCENT_PRESETS) {
    for (const scheme of SCHEMES) {
      const { adjustments } = derive(
        { ...DEFAULT_APPEARANCE, accent: { preset: preset.id } },
        scheme,
      )
      assert.deepEqual(adjustments, [], `${preset.id} ${scheme}`)
    }
  }
  const light = derive(
    { ...DEFAULT_APPEARANCE, accent: { custom: '#ffffaa' } },
    'light',
  )
  assert.deepEqual(light.adjustments, [
    { control: 'accent', kind: 'lightness', reason: 'accent-contrast' },
  ])
  // The stored choice is not touched by the adjustment: the same input gives the same output.
  assert.deepEqual(
    derive({ ...DEFAULT_APPEARANCE, accent: { custom: '#ffffaa' } }, 'light'),
    light,
  )
})

test('window and container tints change only their own surfaces', () => {
  const base = derive(DEFAULT_APPEARANCE, 'light').tokens
  const warmWindow = derive(
    { ...DEFAULT_APPEARANCE, window: { preset: 'warm' } },
    'light',
  ).tokens
  assert.notEqual(warmWindow.background, base.background)
  assert.equal(warmWindow.card, base.card)
  const coolContainer = derive(
    { ...DEFAULT_APPEARANCE, container: { preset: 'cool' } },
    'light',
  ).tokens
  assert.equal(coolContainer.background, base.background)
  for (const name of ['card', 'popover', 'sidebar'] as const) {
    assert.notEqual(coolContainer[name], base[name], name)
  }
})
