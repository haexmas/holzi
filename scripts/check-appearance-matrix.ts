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

for (const control of [
  'accent',
  'window',
  'container',
  'text',
  'component',
] as const) {
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

test('all four tints at the extreme at once, for three hues, keep every pair readable', () => {
  const failures: string[] = []
  const stops = ['#ff0000', '#00ff00', '#0000ff']
  const grey = ['#000000', '#ffffff']
  for (const accent of [...stops, ...grey]) {
    for (const tint of [...stops, ...grey, '#ffff00', '#ff00ff']) {
      for (const scheme of SCHEMES) {
        failures.push(
          ...measure(
            {
              ...DEFAULT_APPEARANCE,
              accent: { custom: accent },
              window: { custom: tint },
              container: { custom: tint },
              text: { custom: tint },
              component: { custom: tint },
            },
            scheme,
            `all ${tint} accent ${accent}`,
          ),
        )
      }
    }
  }
  assert.deepEqual(failures, [])
})

test('text and component tints change only their own tokens', () => {
  const base = derive(DEFAULT_APPEARANCE, 'light').tokens
  const warmText = derive(
    { ...DEFAULT_APPEARANCE, text: { preset: 'warm' } },
    'light',
  ).tokens
  assert.notEqual(warmText.foreground, base.foreground)
  assert.equal(warmText.background, base.background)
  assert.equal(warmText.muted, base.muted)
  const coolComponent = derive(
    { ...DEFAULT_APPEARANCE, component: { preset: 'cool' } },
    'light',
  ).tokens
  for (const name of ['secondary', 'muted', 'accent', 'input', 'border']) {
    assert.notEqual(
      coolComponent[name as TokenName],
      base[name as TokenName],
      name,
    )
  }
  assert.equal(coolComponent.background, base.background)
  assert.equal(coolComponent.card, base.card)
  assert.equal(coolComponent.foreground, base.foreground)
})

test('the window hint is not part of the tokens', () => {
  for (const scheme of SCHEMES) {
    assert.deepEqual(
      derive({ ...DEFAULT_APPEARANCE, windowHint: true }, scheme),
      derive(DEFAULT_APPEARANCE, scheme),
    )
  }
})

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

test('the accent keeps its hue in both schemes and is derived in lightness for each', () => {
  for (const preset of ACCENT_PRESETS.filter((p) => p.c > 0)) {
    const appearance = { ...DEFAULT_APPEARANCE, accent: { preset: preset.id } }
    const light = parseOklch(derive(appearance, 'light').tokens.primary)!
    const dark = parseOklch(derive(appearance, 'dark').tokens.primary)!
    assert.equal(light.h, dark.h, preset.id)
    assert.notEqual(light.l, dark.l, preset.id)
  }
})

test('reset: the default appearance derives the same as the default tokens', () => {
  for (const scheme of SCHEMES) {
    assert.deepEqual(
      derive({ ...DEFAULT_APPEARANCE }, scheme),
      derive(DEFAULT_APPEARANCE, scheme),
    )
  }
})
