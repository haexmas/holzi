// `pnpm check:vault-data` (spec 024 FR-032): the subscriber hub behind live vault data. Pure
// module, loaded with Node type-stripping like check-settings.ts.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  concerns,
  createVaultDataHub,
  matchesTable,
} from '../src/lib/sync/vaultData.ts'

function unexpected(error: unknown): never {
  return assert.fail(`no handler should fail here: ${String(error)}`)
}

test('a pattern is a table name or a prefix ending in a star', () => {
  assert.equal(matchesTable('preferences', 'preferences'), true)
  assert.equal(matchesTable('preferences', 'preferences_extra'), false)
  assert.equal(matchesTable('abc__notes__*', 'abc__notes__items'), true)
  assert.equal(matchesTable('abc__notes__*', 'abc__other__items'), false)
})

test('a change concerns a subscriber when any changed table matches', () => {
  assert.equal(concerns(['preferences'], ['chat_threads', 'preferences']), true)
  assert.equal(concerns(['preferences'], ['chat_threads']), false)
  assert.equal(concerns([], ['preferences']), false)
})

test('a change reaches only the subscribers whose tables it touches', async () => {
  const hub = createVaultDataHub(unexpected)
  const seen: string[] = []
  hub.subscribe({
    tables: ['preferences'],
    handler: () => void seen.push('prefs'),
  })
  hub.subscribe({
    tables: ['chat_threads'],
    handler: () => void seen.push('threads'),
  })

  await hub.dispatch(['preferences'])

  assert.deepEqual(seen, ['prefs'])
})

test('an ended subscription hears nothing more', async () => {
  const hub = createVaultDataHub(unexpected)
  let calls = 0
  const unsubscribe = hub.subscribe({
    tables: ['preferences'],
    handler: () => void calls++,
  })

  await hub.dispatch(['preferences'])
  unsubscribe()
  await hub.dispatch(['preferences'])

  assert.equal(calls, 1)
  assert.equal(hub.size, 0)
})

test('a handler never overlaps itself and runs once more for changes that arrive meanwhile', async () => {
  const hub = createVaultDataHub(unexpected)
  let active = 0
  let maxActive = 0
  let calls = 0
  let release: () => void = () => {}
  hub.subscribe({
    tables: ['preferences'],
    handler: async () => {
      calls++
      active++
      maxActive = Math.max(maxActive, active)
      await new Promise<void>((resolve) => {
        release = resolve
      })
      active--
    },
  })

  const first = hub.dispatch(['preferences'])
  // Three more changes while the first reload runs fold into one more reload.
  const rest = [
    hub.dispatch(['preferences']),
    hub.dispatch(['preferences']),
    hub.dispatch(['preferences']),
  ]
  release()
  await new Promise((resolve) => setImmediate(resolve))
  release()
  await Promise.all([first, ...rest])

  assert.equal(calls, 2)
  assert.equal(maxActive, 1)
})

test('a failing handler is reported and does not stop the others', async () => {
  const errors: unknown[] = []
  const hub = createVaultDataHub((error) => errors.push(error))
  const failure = new Error('read failed')
  let other = 0
  hub.subscribe({
    tables: ['preferences'],
    handler: () => {
      throw failure
    },
  })
  hub.subscribe({ tables: ['preferences'], handler: () => void other++ })

  await hub.dispatch(['preferences'])

  assert.deepEqual(errors, [failure])
  assert.equal(other, 1)
})

test('a failing handler runs again on the next change', async () => {
  const hub = createVaultDataHub(() => {})
  let calls = 0
  hub.subscribe({
    tables: ['preferences'],
    handler: () => {
      calls++
      if (calls === 1) throw new Error('first read fails')
    },
  })

  await hub.dispatch(['preferences'])
  await hub.dispatch(['preferences'])

  assert.equal(calls, 2)
})
