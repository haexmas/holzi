// Part of `pnpm check:passwords` (spec 034-password-manager, FR-027, SC-005): the one action of the
// password manager in the catalog (src/lib/actions/passwordsActions.ts). The built-in agent can
// search titles, tags and folders; no action reads, copies or changes a secret.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  isBuiltinAgentAction,
  listAgentActions,
} from '../src/lib/actions/agentTools.ts'
import { ALL_ACTIONS } from '../src/lib/actions/catalog.ts'
import {
  PASSWORDS_ACTIONS,
  projectSearchResult,
} from '../src/lib/actions/passwordsActions.ts'
import { isSchemaInSubset, validate } from '../src/lib/actions/schema.ts'
import type { JsonSchema } from '../src/lib/actions/types.ts'
import { catalogRunner } from './lib/actions-harness.ts'

const titleOf = (key: string, locale: string) => `${locale}:${key}`

test('the search is offered to the built-in agent, and it is the only password manager action', () => {
  const offered = listAgentActions(ALL_ACTIONS, titleOf).map((d) => d.actionId)
  assert.ok(offered.includes('passwords.items.search'))
  const own = ALL_ACTIONS.filter((a) => a.id.startsWith('passwords.'))
  assert.deepEqual(
    own.map((a) => a.id),
    ['passwords.items.search'],
    'no action reads, copies or changes a secret',
  )
  assert.equal(PASSWORDS_ACTIONS.length, 1)
  const action = own[0]!
  assert.equal(action.scope, 'passwords.read')
  assert.equal(action.effect, 'read')
  assert.ok(isBuiltinAgentAction(action))
})

const SECRET_NAME =
  /secret|private|password|passphrase|token|apiKey|credential/i

function propertyNames(schema: JsonSchema): string[] {
  return [
    ...Object.entries(schema.properties ?? {}).flatMap(([name, child]) => [
      name,
      ...propertyNames(child),
    ]),
    ...(schema.items ? propertyNames(schema.items) : []),
  ]
}

test('the schemas are in the supported subset and no property names a secret', () => {
  const action = PASSWORDS_ACTIONS[0]!
  assert.ok(isSchemaInSubset(action.input))
  assert.ok(isSchemaInSubset(action.result))
  for (const schema of [action.input, action.result]) {
    for (const name of propertyNames(schema))
      assert.doesNotMatch(name, SECRET_NAME, name)
  }
  assert.deepEqual(
    Object.keys(
      action.result.properties?.items?.items?.properties ?? {},
    ).sort(),
    ['folder', 'hasTotp', 'id', 'tags', 'title'],
    'a username and an address are not part of the answer',
  )
})

test('the input is validated: limit is an integer, unknown fields are refused', () => {
  const input = PASSWORDS_ACTIONS[0]!.input
  assert.ok(validate(input, {}).ok)
  assert.ok(validate(input, { query: 'git', tag: 'work', limit: 5 }).ok)
  assert.equal(validate(input, { limit: 'many' }).ok, false)
  assert.equal(validate(input, { password: 'x' }).ok, false)
})

test('the handler answer is projected to the five fields whatever the backend sent', () => {
  const MARKER = 'SECRET-MARKER-ACTION'
  const projected = projectSearchResult({
    items: [
      {
        id: 'i1',
        title: 'Mail',
        tags: ['work', 7],
        folder: 'Private',
        hasTotp: true,
        username: `alice-${MARKER}`,
        url: `https://${MARKER}.invalid`,
        password: MARKER,
      },
      { id: 'i2', title: null, tags: [], folder: null, hasTotp: false },
      { title: 'no id' },
    ],
  })
  assert.deepEqual(projected, {
    items: [
      {
        id: 'i1',
        title: 'Mail',
        tags: ['work'],
        folder: 'Private',
        hasTotp: true,
      },
      { id: 'i2', tags: [], hasTotp: false },
    ],
  })
  assert.ok(!JSON.stringify(projected).includes(MARKER))
  assert.deepEqual(projectSearchResult(null), { items: [] })
  assert.deepEqual(projectSearchResult({ items: 'x' }), { items: [] })
})

test('the action runs for a built-in agent caller and records one handler call', async () => {
  const calls: string[] = []
  const { runAction } = catalogRunner(calls)
  const outcome = await runAction(
    'passwords.items.search',
    {},
    { kind: 'builtinAgent' },
  )
  assert.equal(outcome.ok, true)
  assert.equal(calls.length, 1)
})
