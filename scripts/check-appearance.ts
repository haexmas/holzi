// Run with `node --test scripts/check-appearance*.ts`. Spec 035-appearance-and-fields: the colour
// maths, the stored appearance, the colour fields and the defaults in `tailwind.css` (the contrast of
// every colour field is in `check-appearance-matrix.ts`, the file in `check-appearance-file.ts`).
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { contrast, contrastRgb } from '../src/lib/appearance/contrast.ts'
import { derive } from '../src/lib/appearance/derive.ts'
import {
  formatOklch,
  mapToGamut,
  oklchToRgb,
  parseHex,
  parseOklch,
  rgbToOklch,
  toHex,
} from '../src/lib/appearance/oklch.ts'
import {
  ACCENT_PRESETS,
  TINT_PRESETS,
  presetKey,
} from '../src/lib/appearance/presets.ts'
import {
  DEFAULT_APPEARANCE,
  parseAppearance,
  serializeAppearance,
} from '../src/lib/appearance/schema.ts'
import {
  TOKEN_NAMES,
  TOKEN_CONTROL,
  type Scheme,
} from '../src/lib/appearance/tokens.ts'

const close = (actual: number, expected: number, tolerance = 0.002) =>
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${actual} is not within ${tolerance} of ${expected}`,
  )

test('colour maths: known colours and the round trip', () => {
  const white = rgbToOklch(parseHex('#ffffff')!)
  close(white.l, 1)
  close(white.c, 0)
  const black = rgbToOklch(parseHex('#000000')!)
  close(black.l, 0)
  const cyan = rgbToOklch(parseHex('#00ffff')!)
  close(cyan.l, 0.905, 0.005)
  close(cyan.h, 195, 2)
  for (const hex of ['#00b3a4', '#336699', '#ffcc00', '#808080', '#ff0000']) {
    assert.equal(toHex(oklchToRgb(rgbToOklch(parseHex(hex)!))), hex)
  }
  assert.equal(parseHex('#12'), null)
  assert.equal(parseHex('00ffff'), null)
  assert.equal(parseHex('#00ffgg'), null)
})

test('gamut mapping keeps lightness and hue and lowers chroma', () => {
  const mapped = mapToGamut({ l: 0.65, c: 0.4, h: 180 })
  assert.equal(mapped.l, 0.65)
  assert.equal(mapped.h, 180)
  assert.ok(mapped.c > 0.05 && mapped.c < 0.4)
  assert.deepEqual(mapToGamut({ l: 0.5, c: 0.02, h: 60 }), {
    l: 0.5,
    c: 0.02,
    h: 60,
  })
})

test('oklch text round trip: three decimals, grey hue 0', () => {
  assert.equal(
    formatOklch({ l: 0.65, c: 0.17, h: 180 }),
    'oklch(0.65 0.17 180)',
  )
  assert.equal(formatOklch({ l: 0.955, c: 0, h: 123 }), 'oklch(0.955 0 0)')
  assert.deepEqual(parseOklch('oklch(0.7 0.127 180)'), {
    l: 0.7,
    c: 0.127,
    h: 180,
  })
  assert.equal(parseOklch('rgb(0 0 0)'), null)
})

test('WCAG contrast of known pairs', () => {
  close(contrastRgb([0, 0, 0], [1, 1, 1]), 21, 0.01)
  close(contrastRgb([0.5, 0.5, 0.5], [0.5, 0.5, 0.5]), 1, 0.001)
  close(contrast({ l: 0, c: 0, h: 0 }, { l: 1, c: 0, h: 0 }), 21, 0.01)
})

test('parseAppearance: lenient (FR-020)', () => {
  assert.deepEqual(parseAppearance(null), DEFAULT_APPEARANCE)
  assert.deepEqual(parseAppearance('not json'), DEFAULT_APPEARANCE)
  assert.deepEqual(parseAppearance('[]'), DEFAULT_APPEARANCE)
  assert.deepEqual(parseAppearance('{"v":2}'), DEFAULT_APPEARANCE)
  // One invalid field falls back to its own default and the others stay.
  const mixed = parseAppearance(
    JSON.stringify({
      v: 1,
      accent: { preset: 'blue' },
      window: { preset: 'nope' },
      container: { custom: '#ABCDEF' },
      text: { custom: '#12' },
      component: { preset: 'warm', extra: 1 },
      windowHint: 'yes',
      unknown: true,
    }),
  )
  assert.deepEqual(mixed, {
    v: 1,
    accent: { preset: 'blue' },
    window: DEFAULT_APPEARANCE.window,
    container: { custom: '#abcdef' },
    text: DEFAULT_APPEARANCE.text,
    component: DEFAULT_APPEARANCE.component,
    windowHint: false,
  })
  // The accent row and the tint row differ: `rose` is a tint, not an accent.
  assert.deepEqual(
    parseAppearance('{"v":1,"accent":{"preset":"rose"}}').accent,
    DEFAULT_APPEARANCE.accent,
  )
})

test('serialize keeps known fields in a fixed order and round-trips', () => {
  const appearance = {
    ...DEFAULT_APPEARANCE,
    accent: { custom: '#336699' },
    windowHint: true,
  }
  assert.deepEqual(parseAppearance(serializeAppearance(appearance)), appearance)
  assert.deepEqual(Object.keys(JSON.parse(serializeAppearance(appearance))), [
    'v',
    'accent',
    'window',
    'container',
    'text',
    'component',
    'windowHint',
  ])
})

type Locale = { [key: string]: Locale | string }
const locale = (name: string): Locale =>
  JSON.parse(readFileSync(`src/i18n/locales/${name}.json`, 'utf8'))
const lookup = (tree: Locale, key: string): unknown =>
  key.split('.').reduce<unknown>((node, part) => {
    return typeof node === 'object' && node !== null
      ? (node as Locale)[part]
      : undefined
  }, tree)

test('every colour field has a name in German and English; ids are unique', () => {
  for (const row of [ACCENT_PRESETS, TINT_PRESETS]) {
    assert.equal(new Set(row.map((preset) => preset.id)).size, row.length)
  }
  const ids = new Set([...ACCENT_PRESETS, ...TINT_PRESETS].map((p) => p.id))
  for (const name of ['de', 'en']) {
    const tree = locale(name)
    for (const id of ids) {
      assert.equal(
        typeof lookup(tree, presetKey(id)),
        'string',
        `${name}: ${presetKey(id)}`,
      )
    }
  }
  assert.equal(ACCENT_PRESETS[0]!.id, 'sky')
})

/** The `--name: oklch(...)` declarations of the first block that starts with `selector`. */
function declarations(css: string, selector: string): Record<string, string> {
  const start = css.indexOf(`\n${selector} {`)
  assert.ok(start >= 0, `no ${selector} block in tailwind.css`)
  const block = css.slice(start, css.indexOf('\n}', start))
  return Object.fromEntries(
    [...block.matchAll(/--([a-z-]+):\s*(oklch\([^)]*\))/g)].map((m) => [
      m[1]!,
      m[2]!,
    ]),
  )
}

test('derive(default) is exactly the default tokens of tailwind.css, in both schemes', () => {
  const css = readFileSync('src/assets/css/tailwind.css', 'utf8')
  const blocks: Record<Scheme, Record<string, string>> = {
    light: declarations(css, ':root'),
    dark: declarations(css, '.dark'),
  }
  for (const scheme of ['light', 'dark'] as const) {
    const { tokens, adjustments } = derive(DEFAULT_APPEARANCE, scheme)
    assert.deepEqual(
      adjustments,
      [],
      `${scheme}: the defaults need no adjustment`,
    )
    for (const name of TOKEN_NAMES) {
      const expected = parseOklch(blocks[scheme][name] ?? '')
      assert.ok(expected, `${scheme}: --${name} is missing in tailwind.css`)
      assert.equal(
        tokens[name],
        formatOklch(expected),
        `${scheme}: --${name} (${TOKEN_CONTROL[name]})`,
      )
    }
  }
})
