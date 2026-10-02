// Part of `pnpm check:passwords` (spec 034-password-manager, US3, FR-013, FR-014, SC-004,
// research R10): the password generator (src/lib/passwords/generator.ts) with a deterministic
// random source, so every run draws the same numbers.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  generatePassword,
  randomIndex,
  type GeneratorConfig,
  type RandomSource,
} from '../src/lib/passwords/generator.ts'

/** mulberry32: a small, well mixed generator of 32 bit numbers from a seed. */
function seeded(seed: number): RandomSource {
  let state = seed >>> 0
  return (buffer) => {
    for (let i = 0; i < buffer.length; i += 1) {
      state = (state + 0x6d2b79f5) >>> 0
      let t = state
      t = Math.imul(t ^ (t >>> 15), t | 1)
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
      buffer[i] = (t ^ (t >>> 14)) >>> 0
    }
  }
}

const BASE: GeneratorConfig = {
  length: 20,
  uppercase: true,
  lowercase: true,
  numbers: true,
  symbols: false,
  excludeChars: '',
  usePattern: false,
  pattern: '',
}

const CLASSES: Record<string, RegExp> = {
  uppercase: /[A-Z]/,
  lowercase: /[a-z]/,
  numbers: /[0-9]/,
  symbols: /[!@#$%^&*()_+\-=[\]{}|;:,.<>?]/,
}

function value(config: GeneratorConfig, random: RandomSource): string {
  const result = generatePassword(config, random)
  assert.ok(
    'value' in result,
    `expected a value, got ${JSON.stringify(result)}`,
  )
  return result.value
}

test('1,000 outputs per configuration have the length and every chosen class', () => {
  const random = seeded(1)
  const configs: GeneratorConfig[] = [
    { ...BASE },
    { ...BASE, length: 4, symbols: true },
    { ...BASE, length: 3, uppercase: false, symbols: true },
    { ...BASE, length: 64, lowercase: false, numbers: false, symbols: true },
    { ...BASE, length: 1, uppercase: false, numbers: false },
    { ...BASE, length: 256 },
  ]
  for (const config of configs) {
    for (let run = 0; run < 1000; run += 1) {
      const out = value(config, random)
      assert.equal(out.length, config.length)
      for (const [name, pattern] of Object.entries(CLASSES)) {
        const chosen = config[name as keyof GeneratorConfig] === true
        if (chosen) assert.match(out, pattern, `${name} missing in ${out}`)
        else assert.doesNotMatch(out, pattern, `${name} present in ${out}`)
      }
    }
  }
})

test('excluded characters never appear, in any class', () => {
  const random = seeded(2)
  const config = { ...BASE, symbols: true, excludeChars: 'lO0I1|{}' }
  for (let run = 0; run < 1000; run += 1) {
    assert.doesNotMatch(value(config, random), /[lO0I1|{}]/)
  }
})

test('pattern mode follows c C v V d a A s, literals and a backslash escape', () => {
  const random = seeded(3)
  const pattern = 'cvCV-da-As\\d\\\\x'
  for (let run = 0; run < 500; run += 1) {
    const out = value({ ...BASE, usePattern: true, pattern }, random)
    assert.match(
      out,
      /^[bcdfghjklmnpqrstvwxyz][aeiou][BCDFGHJKLMNPQRSTVWXYZ][AEIOU]-[0-9][a-z]-[A-Z][!@#$%^&*()_+\-=[\]{}|;:,.<>?]d\\x$/,
    )
  }
})

test('pattern mode ignores the length and the excluded characters', () => {
  const random = seeded(4)
  const config = {
    ...BASE,
    length: 3,
    usePattern: true,
    pattern: 'dddddddd',
    excludeChars: '0123456789',
  }
  const out = value(config, random)
  assert.equal(out.length, 8)
  assert.match(out, /^[0-9]{8}$/)
})

test('a choice that allows no output returns an error code, not a value', () => {
  const random = seeded(5)
  const code = (config: GeneratorConfig) => {
    const result = generatePassword(config, random)
    assert.ok(
      'error' in result,
      `expected an error, got ${JSON.stringify(result)}`,
    )
    return result.error
  }
  assert.equal(code({ ...BASE, length: 2 }), 'length_below_classes')
  assert.equal(
    code({ ...BASE, uppercase: false, lowercase: false, numbers: false }),
    'no_class',
  )
  assert.equal(code({ ...BASE, length: 0 }), 'length_range')
  assert.equal(code({ ...BASE, length: 257 }), 'length_range')
  assert.equal(code({ ...BASE, length: 10.5 }), 'length_range')
  assert.equal(
    code({
      ...BASE,
      uppercase: false,
      lowercase: false,
      symbols: false,
      excludeChars: '0123456789',
    }),
    'class_empty',
    'a chosen class that the exclusion leaves empty cannot be satisfied',
  )
  assert.equal(
    code({ ...BASE, usePattern: true, pattern: '' }),
    'pattern_empty',
  )
})

test('rejection sampling skips the biased tail of the random range', () => {
  // For 3 choices the biased tail starts at 4294967295: that draw is dropped, the next is used.
  const queue = [0xffffffff, 7]
  const source: RandomSource = (buffer) => {
    buffer[0] = queue.shift() ?? 0
  }
  assert.equal(randomIndex(3, source), 7 % 3)
  assert.equal(queue.length, 0)
  assert.equal(
    randomIndex(1, () => assert.fail('one choice needs no randomness')),
    0,
  )
})

test('the choice is uniform: chi-square over 60,000 draws from a 7 character set stays low', () => {
  const random = seeded(6)
  const counts = new Array<number>(7).fill(0)
  const draws = 60000
  for (let i = 0; i < draws; i += 1) counts[randomIndex(7, random)]! += 1
  const expected = draws / 7
  const chiSquare = counts.reduce(
    (sum, n) => sum + (n - expected) ** 2 / expected,
    0,
  )
  // 6 degrees of freedom: 22.5 is the 99.9 % point; the draws are fixed, so this never flakes.
  assert.ok(chiSquare < 22.5, `chi-square ${chiSquare}`)
})

test('the guaranteed characters are shuffled with the same random source', () => {
  // With a source that always draws 0 the shuffle is deterministic: the same call twice agrees,
  // and the output is not "one of each class in class order" for every seed.
  const same = () => (buffer: Uint32Array) => buffer.fill(0)
  assert.equal(
    value({ ...BASE, length: 6 }, same()),
    value({ ...BASE, length: 6 }, same()),
  )
  const positions = new Set<number>()
  for (let seed = 0; seed < 50; seed += 1) {
    const out = value({ ...BASE, length: 3, symbols: false }, seeded(seed))
    positions.add(out.search(/[0-9]/))
  }
  assert.ok(positions.size > 1, 'the digit moves around')
})

test('without a random source it uses the platform source and still satisfies the rules', () => {
  for (let run = 0; run < 50; run += 1) {
    const result = generatePassword({ ...BASE, symbols: true, length: 16 })
    assert.ok('value' in result)
    assert.equal(result.value.length, 16)
  }
})
