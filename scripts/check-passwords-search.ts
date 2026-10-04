// Part of `pnpm check:passwords` (spec 034-password-manager, FR-007, SC-002, research R11): the
// search over the headers of the entries (src/lib/passwords/search.ts). It looks at exactly title,
// username, URL and tag names, never at a note, a password or another field.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  filterHeaders,
  fold,
  matchesQuery,
  type SearchableHeader,
} from '../src/lib/passwords/search.ts'

function header(
  overrides: Partial<SearchableHeader> & { id: string },
): SearchableHeader {
  return {
    title: null,
    username: null,
    url: null,
    tags: [],
    ...overrides,
  }
}

const MAIL = header({
  id: 'mail',
  title: 'Mail Account',
  username: 'alice@example.invalid',
  url: 'https://mail.example.invalid/login',
  tags: [{ id: 't-work', name: 'Arbeit' }],
})
const BANK = header({
  id: 'bank',
  title: 'Bänk Konto',
  username: 'bob',
  url: 'https://bank.example.invalid',
  tags: [{ id: 't-fin', name: 'Finanzen' }],
})

test('a word part of title, username, URL and tag names matches', () => {
  assert.ok(matchesQuery(MAIL, 'acc'), 'title part')
  assert.ok(matchesQuery(MAIL, 'alice'), 'username part')
  assert.ok(matchesQuery(MAIL, 'mail.example'), 'URL part')
  assert.ok(matchesQuery(MAIL, 'arbe'), 'tag name part')
  assert.ok(!matchesQuery(MAIL, 'bob'))
})

test('case and diacritics do not matter', () => {
  assert.ok(matchesQuery(BANK, 'BANK'))
  assert.ok(matchesQuery(BANK, 'bank'))
  assert.ok(matchesQuery(BANK, 'bänk'), 'with the umlaut')
  assert.ok(matchesQuery(BANK, 'konto'))
  assert.equal(fold('Bänk'), fold('Banke'.slice(0, 4)))
  // A decomposed umlaut finds the composed one.
  assert.ok(matchesQuery(BANK, 'bänk'))
})

test('several words must all match, in any of the fields', () => {
  assert.ok(matchesQuery(MAIL, 'mail alice'))
  assert.ok(matchesQuery(MAIL, 'arbeit login'))
  assert.ok(!matchesQuery(MAIL, 'mail bob'))
})

test('a note, a password or any other field is never searched', () => {
  const sneaky = {
    ...header({ id: 'x', title: 'Plain' }),
    note: 'SECRET-MARKER-SEARCH-note',
    password: 'SECRET-MARKER-SEARCH-password',
    keyValues: [{ key: 'PIN', value: 'SECRET-MARKER-SEARCH-pin' }],
  }
  assert.ok(!matchesQuery(sneaky, 'SECRET-MARKER-SEARCH'))
  assert.ok(!matchesQuery(sneaky, 'marker'))
  assert.deepEqual(filterHeaders([sneaky], { query: 'secret-marker' }), [])
})

test('an entry without a title is found by its username and by its URL', () => {
  const untitled = header({
    id: 'untitled',
    username: 'carol',
    url: 'https://shop.example.invalid',
  })
  assert.ok(matchesQuery(untitled, 'carol'))
  assert.ok(matchesQuery(untitled, 'shop'))
  assert.ok(
    !matchesQuery(untitled, 'untitled'),
    'the placeholder is not a value',
  )
  assert.ok(!matchesQuery(untitled, 'ohne titel'))
})

test('an empty query returns everything, a tag id filters by tag', () => {
  const all = [MAIL, BANK]
  assert.deepEqual(filterHeaders(all, { query: '' }), all)
  assert.deepEqual(filterHeaders(all, { query: '   ' }), all)
  assert.deepEqual(filterHeaders(all, { query: '', tagId: 't-fin' }), [BANK])
  assert.deepEqual(filterHeaders(all, { query: 'example', tagId: 't-work' }), [
    MAIL,
  ])
  assert.deepEqual(filterHeaders(all, { query: 'nothing' }), [])
})

test('5,000 headers are filtered well inside one second', () => {
  const many: SearchableHeader[] = Array.from({ length: 5000 }, (_, i) =>
    header({
      id: `i${i}`,
      title: `Entry ${i} Überprüfung`,
      username: `user${i}`,
      url: `https://site${i}.example.invalid`,
      tags: [{ id: `t${i % 20}`, name: `Tag ${i % 20}` }],
    }),
  )
  const started = performance.now()
  const hits = filterHeaders(many, { query: 'uberprufung entry 49' })
  const elapsed = performance.now() - started
  assert.ok(hits.length > 0)
  // A coarse sanity bound (SC-002), not a tight timing.
  assert.ok(elapsed < 1000, `took ${elapsed} ms`)
})

test('a reference placeholder is not searchable as text (spec 036)', () => {
  const id = '7c1e0000-0000-4000-8000-000000000001'
  const reference = header({
    id: 'ref',
    title: 'Zweit',
    username: `admin-{$${id}:username}`,
    url: `{$${id}:extra:a\\}b}`,
  })
  assert.equal(matchesQuery(reference, 'username'), false)
  assert.equal(matchesQuery(reference, '7c1e'), false)
  assert.equal(matchesQuery(reference, 'extra'), false)
  assert.equal(matchesQuery(reference, 'admin'), true)
  assert.equal(matchesQuery(reference, 'zweit'), true)
  // Text that only looks like a placeholder stays searchable.
  const plain = header({ id: 'plain', username: 'price {$ 5' })
  assert.equal(matchesQuery(plain, '{$'), true)
})
