// Run with `node --test scripts/check-appearance*.ts`. Spec 035-appearance-and-fields, FR-021 and
// contracts/appearance-file.md: the file round-trips, and every fault is refused completely with its
// own cause — nothing is repaired.
import assert from 'node:assert/strict'
import test from 'node:test'
import {
  DEFAULT_APPEARANCE,
  FILE_MAX_BYTES,
  exportFile,
  parseAppearanceFile,
  type Appearance,
} from '../src/lib/appearance/schema.ts'

const custom: Appearance = {
  v: 1,
  accent: { custom: '#336699' },
  window: { preset: 'warm' },
  container: { preset: 'cool' },
  text: { custom: '#102030' },
  component: { preset: 'rose' },
  windowHint: true,
}

/** A valid file as an object, changed by `edit`, as text. */
function file(
  edit: (
    value: Record<string, unknown> & { appearance: Record<string, unknown> },
  ) => void = () => {},
): string {
  const value = JSON.parse(exportFile(custom, 'dark'))
  edit(value)
  return JSON.stringify(value)
}

test('export and import give the same appearance and colour scheme', () => {
  for (const [appearance, scheme] of [
    [custom, 'dark'],
    [DEFAULT_APPEARANCE, 'system'],
    [{ ...custom, windowHint: false }, 'light'],
  ] as const) {
    const result = parseAppearanceFile(exportFile(appearance, scheme))
    assert.deepEqual(result, { ok: true, appearance, colorScheme: scheme })
  }
})

test('the file holds only colours, the scheme and the hint', () => {
  const value = JSON.parse(exportFile(custom, 'dark'))
  assert.deepEqual(Object.keys(value), [
    'format',
    'v',
    'colorScheme',
    'appearance',
  ])
  assert.deepEqual(Object.keys(value.appearance), [
    'v',
    'accent',
    'window',
    'container',
    'text',
    'component',
    'windowHint',
  ])
})

const refused: [string, string, string, string?][] = [
  ['not JSON', '{nope', 'notJson'],
  ['empty', '', 'notJson'],
  [
    'larger than 16 KiB',
    file((v) => (v.pad = 'x'.repeat(FILE_MAX_BYTES))),
    'notJson',
  ],
  ['a JSON array', '[]', 'notAppearance'],
  ['another format', file((v) => (v.format = 'other')), 'notAppearance'],
  ['no format', file((v) => delete v.format), 'notAppearance'],
  ['unknown version', file((v) => (v.v = 2)), 'version'],
  ['unknown inner version', file((v) => (v.appearance.v = 2)), 'version'],
  [
    'bad colour scheme',
    file((v) => (v.colorScheme = 'auto')),
    'field',
    'colorScheme',
  ],
  [
    'missing colour scheme',
    file((v) => delete v.colorScheme),
    'field',
    'colorScheme',
  ],
  ['extra top-level field', file((v) => (v.name = 'x')), 'field', 'name'],
  [
    'extra appearance field',
    file((v) => (v.appearance.size = 1)),
    'field',
    'appearance.size',
  ],
  [
    'appearance not an object',
    file((v) => ((v as Record<string, unknown>).appearance = 1)),
    'field',
    'appearance',
  ],
  [
    'missing accent',
    file((v) => delete v.appearance.accent),
    'field',
    'appearance.accent',
  ],
  [
    'accent neither preset nor custom',
    file((v) => (v.appearance.accent = {})),
    'field',
    'appearance.accent',
  ],
  [
    'accent with both',
    file((v) => (v.appearance.accent = { preset: 'teal', custom: '#000000' })),
    'field',
    'appearance.accent',
  ],
  [
    'unknown preset',
    file((v) => (v.appearance.accent = { preset: 'nope' })),
    'field',
    'appearance.accent',
  ],
  [
    'tint preset in the accent row',
    file((v) => (v.appearance.accent = { preset: 'rose' })),
    'field',
    'appearance.accent',
  ],
  [
    'accent preset in a tint row',
    file((v) => (v.appearance.window = { preset: 'teal' })),
    'field',
    'appearance.window',
  ],
  [
    'short hex',
    file((v) => (v.appearance.container = { custom: '#12' })),
    'field',
    'appearance.container',
  ],
  [
    'hex with alpha',
    file((v) => (v.appearance.text = { custom: '#11223344' })),
    'field',
    'appearance.text',
  ],
  [
    'hex without #',
    file((v) => (v.appearance.component = { custom: '112233' })),
    'field',
    'appearance.component',
  ],
  [
    'hint not a boolean',
    file((v) => (v.appearance.windowHint = 'yes')),
    'field',
    'appearance.windowHint',
  ],
]

for (const [name, text, reason, field] of refused) {
  test(`refused: ${name}`, () => {
    const result = parseAppearanceFile(text)
    assert.equal(result.ok, false)
    if (!result.ok) {
      assert.equal(result.reason, reason)
      assert.equal(result.field, field)
    }
  })
}

test('a file with one valid and one invalid value changes nothing: the result carries no appearance', () => {
  const result = parseAppearanceFile(
    file((v) => {
      v.appearance.accent = { preset: 'blue' }
      v.appearance.window = { preset: 'nope' }
    }),
  )
  assert.deepEqual(result, {
    ok: false,
    reason: 'field',
    field: 'appearance.window',
  })
})

test('upper-case hex is accepted and stored in lower case', () => {
  const result = parseAppearanceFile(
    file((v) => (v.appearance.accent = { custom: '#ABCDEF' })),
  )
  assert.ok(result.ok)
  if (result.ok)
    assert.deepEqual(result.appearance.accent, { custom: '#abcdef' })
})
