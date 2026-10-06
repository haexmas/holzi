// Part of `pnpm check:passwords`: the entry header's delete button must render the confirmation
// dialog that moves the current entry to the trash.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const entryView = readFileSync('src/components/passwords/EntryView.vue', 'utf8')

test('EntryView wires its delete state to the trash confirmation dialog', () => {
  assert.match(entryView, /<PasswordsDeleteDialog\b/)
  assert.match(entryView, /v-model:open="deleteOpen"/)
  assert.match(entryView, /:targets="\[\{ kind: 'item', id: itemId \}\]"/)
  assert.match(entryView, /@done="leaveDeleted"/)
})
