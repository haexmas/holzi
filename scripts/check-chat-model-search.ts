// `pnpm check:chat-model-search` (spec 031-chat-model-search): the fuzzy model-search filter. Pure
// module, loaded with Node type-stripping like check-settings.ts.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { filterModelGroups, type ModelGroup } from '../src/lib/models/search.ts'

const GROUPS: ModelGroup[] = [
  {
    providerId: 'openai',
    providerName: 'OpenAI',
    models: [
      { id: 'gpt-4o', name: 'GPT-4o' },
      { id: 'gpt-4o-mini', name: 'GPT-4o mini' },
    ],
  },
  {
    providerId: 'anthropic',
    providerName: 'Anthropic',
    models: [{ id: 'claude', name: 'Claude' }],
  },
  {
    providerId: 'local',
    providerName: 'Lokal',
    models: [{ id: 'qwen', name: 'Qwen2.5' }],
  },
]

function names(groups: ModelGroup[]): string[] {
  return groups.flatMap((g) => g.models.map((m) => m.name))
}

test('empty query returns every group unchanged, same order', () => {
  const result = filterModelGroups(GROUPS, '')
  assert.deepEqual(result, GROUPS)
})

test('a non-contiguous query finds the model ("gpt4o" -> "GPT-4o")', () => {
  const result = filterModelGroups(GROUPS, 'gpt4o')
  assert.ok(names(result).includes('GPT-4o'))
})

test('a single-substitution typo still finds the model', () => {
  const result = filterModelGroups(GROUPS, 'clyude')
  assert.ok(names(result).includes('Claude'))
})

test('a query matching only the provider name keeps its models', () => {
  const result = filterModelGroups(GROUPS, 'anthropic')
  assert.deepEqual(names(result), ['Claude'])
})

test('a query with no match returns no groups', () => {
  const result = filterModelGroups(GROUPS, 'zzzzzzzz')
  assert.deepEqual(result, [])
})

test('a group with no matches is dropped, a matching sibling group stays', () => {
  const result = filterModelGroups(GROUPS, 'qwen')
  assert.deepEqual(
    result.map((g) => g.providerId),
    ['local'],
  )
})

test('matches within a group keep their original relative order', () => {
  const result = filterModelGroups(GROUPS, 'gpt')
  const openai = result.find((g) => g.providerId === 'openai')
  assert.deepEqual(
    openai?.models.map((m) => m.id),
    ['gpt-4o', 'gpt-4o-mini'],
  )
})
