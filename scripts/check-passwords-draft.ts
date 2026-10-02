// Part of `pnpm check:passwords` (spec 034-password-manager, US1, research R7): the draft of the
// entry editor (src/lib/passwords/draft.ts). An update sends only what changed, and a secret that
// was not touched never appears in it.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  cloneDraft,
  draftFromDetail,
  type DetailLike,
  emptyDraft,
  isDirty,
  toInput,
  toPatch,
} from '../src/lib/passwords/draft.ts'

const DETAIL: DetailLike = {
  title: 'Mail',
  username: 'alice',
  url: 'https://example.invalid',
  icon: 'lucide:mail',
  color: '#3b82f6',
  tags: [{ name: 'Work' }, { name: 'Bank' }],
  expiresAt: '2030-01-31',
  note: 'a note',
  keyValues: [
    { id: 'k1', key: 'PIN', hasValue: true },
    { id: 'k2', key: 'Empty', hasValue: false },
  ],
}

test('an untouched draft makes an empty patch and is not dirty', () => {
  const initial = draftFromDetail(DETAIL)
  const draft = cloneDraft(initial)
  assert.deepEqual(toPatch(initial, draft), {})
  assert.equal(isDirty(initial, draft), false)
})

test('only the changed fields are in the patch', () => {
  const initial = draftFromDetail(DETAIL)
  const draft = cloneDraft(initial)
  draft.title = 'Work mail'
  draft.note = ''
  assert.deepEqual(toPatch(initial, draft), { title: 'Work mail', note: '' })
  assert.equal(isDirty(initial, draft), true)
})

test('the password is in the patch only when it was replaced', () => {
  const initial = draftFromDetail(DETAIL)
  assert.equal('password' in toPatch(initial, cloneDraft(initial)), false)
  const replaced = cloneDraft(initial)
  replaced.password = { mode: 'set', value: 'new-secret' }
  assert.equal(toPatch(initial, replaced).password, 'new-secret')
  const emptied = cloneDraft(initial)
  emptied.password = { mode: 'set', value: '' }
  assert.equal(
    toPatch(initial, emptied).password,
    '',
    'an empty password is stored as empty',
  )
})

test('removing a date, an icon or a color clears it', () => {
  const initial = draftFromDetail(DETAIL)
  const draft = cloneDraft(initial)
  draft.expiresAt = ''
  draft.icon = null
  draft.color = null
  assert.deepEqual(toPatch(initial, draft), {
    expiresAt: null,
    icon: null,
    color: null,
  })
})

test('the TOTP is replaced, cleared or left alone', () => {
  const initial = draftFromDetail(DETAIL)
  const replace = cloneDraft(initial)
  replace.otp = {
    mode: 'set',
    text: 'JBSWY3DPEHPK3PXP',
    digits: 8,
    period: null,
    algorithm: null,
  }
  assert.deepEqual(toPatch(initial, replace), {
    otpSecret: 'JBSWY3DPEHPK3PXP',
    otpDigits: 8,
  })
  const clear = cloneDraft(initial)
  clear.otp = { mode: 'clear' }
  assert.deepEqual(toPatch(initial, clear), { otpSecret: null })
})

test('tags compare as a set, not by order', () => {
  const initial = draftFromDetail(DETAIL)
  const reordered = cloneDraft(initial)
  reordered.tags = ['Bank', 'Work']
  assert.equal('tags' in toPatch(initial, reordered), false)
  const added = cloneDraft(initial)
  added.tags.push('New')
  assert.deepEqual(toPatch(initial, added).tags, ['Work', 'Bank', 'New'])
})

test('an untouched custom field keeps its stored value, a new one carries its own', () => {
  const initial = draftFromDetail(DETAIL)
  const draft = cloneDraft(initial)
  draft.keyValues[0]!.key = 'PIN code'
  draft.keyValues.push({
    id: null,
    key: 'Recovery',
    value: 'r-1',
    hasStoredValue: false,
  })
  const patch = toPatch(initial, draft)
  assert.deepEqual(patch.keyValues, [
    { id: 'k1', key: 'PIN code', value: undefined },
    { id: 'k2', key: 'Empty', value: undefined },
    { id: undefined, key: 'Recovery', value: 'r-1' },
  ])
  const changed = cloneDraft(initial)
  changed.keyValues[0]!.value = 'new-pin'
  assert.equal(toPatch(initial, changed).keyValues?.[0]?.value, 'new-pin')
})

test('a create input leaves out empty texts and fields without a key', () => {
  const draft = emptyDraft()
  draft.title = 'Shop'
  draft.password = { mode: 'set', value: 'pw' }
  draft.tags = ['Home']
  draft.keyValues = [
    { id: null, key: 'PIN', value: '1234', hasStoredValue: false },
    { id: null, key: '  ', value: 'dropped', hasStoredValue: false },
  ]
  const input = toInput(draft)
  assert.equal(input.title, 'Shop')
  assert.equal(input.password, 'pw')
  assert.equal(input.username, undefined)
  assert.deepEqual(input.tags, ['Home'])
  assert.deepEqual(input.keyValues, [{ key: 'PIN', value: '1234' }])
  assert.equal('otpSecret' in input, false)
})

test('a create input carries a TOTP secret with its parts', () => {
  const draft = emptyDraft()
  draft.otp = {
    mode: 'set',
    text: 'otpauth://totp/x?secret=ABC',
    digits: null,
    period: 60,
    algorithm: 'SHA256',
  }
  const input = toInput(draft)
  assert.equal(input.otpSecret, 'otpauth://totp/x?secret=ABC')
  assert.equal(input.otpPeriod, 60)
  assert.equal(input.otpAlgorithm, 'SHA256')
  assert.equal('otpDigits' in input, false)
})
