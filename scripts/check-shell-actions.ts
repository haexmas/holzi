// Part of `pnpm check:shell-navigation` (spec 020-tab-navigation): the action core under
// `src/lib/actions/` — the JSON-schema subset validator, the `runAction` pipeline with every error
// code, the guardrail lock for agent callers, and invariants over the shipped action catalog
// (data-model.md invariants 6–7).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { ALL_ACTIONS } from '../src/lib/actions/catalog.ts'
import {
  createActionRunner,
  type ActionHandler,
  type ActionRunnerDeps,
} from '../src/lib/actions/runner.ts'
import { isSchemaInSubset, validate } from '../src/lib/actions/schema.ts'
import { ACTION_SCOPES } from '../src/lib/actions/scopes.ts'
import type {
  ActionCaller,
  JsonSchema,
  ShellActionDefinition,
} from '../src/lib/actions/types.ts'

const USER: ActionCaller = { kind: 'user' }
const BUILTIN: ActionCaller = { kind: 'builtinAgent' }
const EXTERNAL: ActionCaller = { kind: 'externalAgent', agentId: 'agent-1' }

const EMPTY_OBJECT: JsonSchema = { type: 'object', properties: {} }

function action(
  overrides: Partial<ShellActionDefinition> & { id: string },
): ShellActionDefinition {
  return {
    titleKey: `actions.${overrides.id}`,
    description: 'test action',
    input: EMPTY_OBJECT,
    result: EMPTY_OBJECT,
    target: 'none',
    scope: 'shell.layout',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
    ...overrides,
  }
}

const GO: ShellActionDefinition = action({
  id: 'test.go',
  target: 'tab',
  scope: 'shell.navigation',
  input: {
    type: 'object',
    properties: {
      tabId: { type: 'string' },
      steps: { type: 'integer', description: 'signed step count' },
    },
    required: ['steps'],
  },
})
const LOCKED = action({
  id: 'test.locked',
  scope: 'guardrails',
  agentCallable: false,
})
const CHAT_SEND = action({
  id: 'test.chat.send',
  binding: 'tab',
  appId: 'system.chat',
  input: {
    type: 'object',
    properties: { text: { type: 'string' } },
    required: ['text'],
  },
})
const BROKEN = action({ id: 'test.broken' })

type Harness = {
  calls: { id: string; input: Record<string, unknown>; target: unknown }[]
  deps: ActionRunnerDeps
}

function harness(overrides: Partial<ActionRunnerDeps> = {}): Harness {
  const calls: Harness['calls'] = []
  const record =
    (id: string): ActionHandler =>
    ({ input, target }) => {
      calls.push({ id, input, target })
      return { done: id }
    }
  const handlers = new Map<string, ActionHandler>([
    [GO.id, record(GO.id)],
    [LOCKED.id, record(LOCKED.id)],
    [
      BROKEN.id,
      () => {
        throw new Error('boom')
      },
    ],
  ])
  const deps: ActionRunnerDeps = {
    catalog: [GO, LOCKED, CHAT_SEND, BROKEN],
    globalHandler: (id) => handlers.get(id),
    resolveFocus: (target) => (target === 'tab' ? 'focused-tab' : null),
    targetExists: (_target, id) => id === 'focused-tab' || id === 'tab-2',
    awaitTabHandler: async () => record(CHAT_SEND.id),
    ...overrides,
  }
  return { calls, deps }
}

// ---------------------------------------------------------------------------
// Schema subset (schema.ts)
// ---------------------------------------------------------------------------

test('validate accepts matching input and reports the failing field otherwise', () => {
  assert.deepEqual(validate(GO.input, { steps: -2 }), { ok: true })
  const wrongType = validate(GO.input, { steps: 'x' })
  assert.equal(wrongType.ok, false)
  assert.equal(!wrongType.ok && wrongType.field, 'steps')
  const notInteger = validate(GO.input, { steps: 1.5 })
  assert.equal(!notInteger.ok && notInteger.field, 'steps')
  const missing = validate(GO.input, {})
  assert.equal(!missing.ok && missing.field, 'steps')
  const unknown = validate(GO.input, { steps: 1, extra: true })
  assert.equal(!unknown.ok && unknown.field, 'extra')
})

test('validate checks enums, arrays and booleans', () => {
  const schema: JsonSchema = {
    type: 'object',
    properties: {
      mode: { type: 'string', enum: ['manual', 'auto'] },
      ids: { type: 'array', items: { type: 'string' } },
      flag: { type: 'boolean' },
      size: { type: 'number' },
    },
  }
  assert.deepEqual(
    validate(schema, { mode: 'auto', ids: ['a'], flag: true, size: 1.5 }),
    { ok: true },
  )
  const badEnum = validate(schema, { mode: 'plan' })
  assert.equal(!badEnum.ok && badEnum.field, 'mode')
  const badItem = validate(schema, { ids: ['a', 2] })
  assert.equal(!badItem.ok && badItem.field, 'ids[1]')
})

test('isSchemaInSubset rejects keywords outside the subset', () => {
  assert.ok(isSchemaInSubset(GO.input))
  assert.ok(
    !isSchemaInSubset({
      type: 'object',
      oneOf: [],
    } as unknown as JsonSchema),
  )
  assert.ok(!isSchemaInSubset({ type: 'null' } as unknown as JsonSchema))
})

// ---------------------------------------------------------------------------
// runAction pipeline (runner.ts)
// ---------------------------------------------------------------------------

test('runAction runs a global handler with the focused tab for a user caller', async () => {
  const { calls, deps } = harness()
  const outcome = await createActionRunner(deps).runAction(GO.id, { steps: -1 })
  assert.deepEqual(outcome, { ok: true, result: { done: GO.id } })
  assert.deepEqual(calls[0]?.target, { tabId: 'focused-tab' })
})

test('runAction reports unknown_action', async () => {
  const outcome = await createActionRunner(harness().deps).runAction('nope')
  assert.equal(!outcome.ok && outcome.code, 'unknown_action')
})

test('runAction refuses guardrail actions for both agent kinds before the handler runs', async () => {
  for (const caller of [BUILTIN, EXTERNAL]) {
    const { calls, deps } = harness()
    const outcome = await createActionRunner(deps).runAction(
      LOCKED.id,
      {},
      caller,
    )
    assert.equal(!outcome.ok && outcome.code, 'forbidden_for_agents')
    assert.equal(calls.length, 0)
  }
  const { calls, deps } = harness()
  const outcome = await createActionRunner(deps).runAction(LOCKED.id, {}, USER)
  assert.equal(outcome.ok, true)
  assert.equal(calls.length, 1)
})

test('runAction reports invalid_input with the field', async () => {
  const outcome = await createActionRunner(harness().deps).runAction(GO.id, {
    steps: 'x',
  })
  assert.equal(!outcome.ok && outcome.code, 'invalid_input')
  assert.equal(!outcome.ok && outcome.field, 'steps')
})

test('agents must name the target explicitly; users fall back to focus (FR-030)', async () => {
  const runner = createActionRunner(harness().deps)
  const missing = await runner.runAction(GO.id, { steps: -1 }, EXTERNAL)
  assert.equal(!missing.ok && missing.code, 'target_required')
  const explicit = await runner.runAction(
    GO.id,
    { steps: -1, tabId: 'tab-2' },
    EXTERNAL,
  )
  assert.equal(explicit.ok, true)
})

test('runAction reports target_not_found for an unknown explicit target', async () => {
  const outcome = await createActionRunner(harness().deps).runAction(GO.id, {
    steps: -1,
    tabId: 'gone',
  })
  assert.equal(!outcome.ok && outcome.code, 'target_not_found')
})

test('runAction reports target_required when a user has no focus either', async () => {
  const { deps } = harness({ resolveFocus: () => null })
  const outcome = await createActionRunner(deps).runAction(GO.id, { steps: -1 })
  assert.equal(!outcome.ok && outcome.code, 'target_required')
})

test('tab-bound actions wait for their app; no handler in time → app_unavailable', async () => {
  const ok = await createActionRunner(harness().deps).runAction(CHAT_SEND.id, {
    text: 'hi',
  })
  assert.deepEqual(ok, { ok: true, result: { done: CHAT_SEND.id } })
  const { deps } = harness({ awaitTabHandler: async () => null })
  const outcome = await createActionRunner(deps).runAction(CHAT_SEND.id, {
    text: 'hi',
  })
  assert.equal(!outcome.ok && outcome.code, 'app_unavailable')
})

test('a structured backend error keeps its reason and travels raw in error', async () => {
  const raw = { kind: 'InvalidInput', reason: 'bad title' }
  const { deps } = harness({
    globalHandler: () => () => {
      throw raw
    },
  })
  const outcome = await createActionRunner(deps).runAction(GO.id, { steps: 1 })
  assert.equal(!outcome.ok && outcome.message, 'bad title')
  assert.equal(!outcome.ok && outcome.error, raw)
})

test('a throwing handler becomes failed with its message', async () => {
  const outcome = await createActionRunner(harness().deps).runAction(BROKEN.id)
  assert.equal(!outcome.ok && outcome.code, 'failed')
  assert.equal(!outcome.ok && outcome.message, 'boom')
})

test('a global action without a registered handler is failed, not a crash', async () => {
  const { deps } = harness({ globalHandler: () => undefined })
  const outcome = await createActionRunner(deps).runAction(GO.id, { steps: 1 })
  assert.equal(!outcome.ok && outcome.code, 'failed')
})

// ---------------------------------------------------------------------------
// Shipped catalog invariants (catalog.ts)
// ---------------------------------------------------------------------------

test('catalog ids are unique', () => {
  const ids = ALL_ACTIONS.map((a) => a.id)
  assert.equal(new Set(ids).size, ids.length)
})

test('every catalog schema stays inside the subset', () => {
  for (const a of ALL_ACTIONS) {
    assert.ok(isSchemaInSubset(a.input), `${a.id} input`)
    assert.ok(isSchemaInSubset(a.result), `${a.id} result`)
  }
})

test('every catalog scope exists; guardrails are never agent-callable', () => {
  const scopes = new Set(ACTION_SCOPES.map((s) => s.id))
  for (const a of ALL_ACTIONS) {
    assert.ok(scopes.has(a.scope), `${a.id} scope ${a.scope}`)
    if (a.scope === 'guardrails') assert.equal(a.agentCallable, false, a.id)
  }
})

test('tab-bound actions name their app; targeted actions declare their target field', () => {
  const field = { tab: 'tabId', window: 'windowId', workspace: 'workspaceId' }
  for (const a of ALL_ACTIONS) {
    if (a.binding === 'tab') assert.ok(a.appId, `${a.id} appId`)
    if (a.target !== 'none') {
      assert.ok(
        a.input.properties?.[field[a.target]],
        `${a.id} ${field[a.target]}`,
      )
    }
  }
})

// ---------------------------------------------------------------------------
// Catalog-wide agent policy (SC-008, FR-032)
// ---------------------------------------------------------------------------

/** These must stay user-only; a test failure here means a guardrail left the lock. */
const EXPECTED_GUARDRAILS = [
  'chat.approval.decide',
  'chat.permissionMode.set',
  'chat.modelIntegrity.decide',
  'settings.delegate.connectProvider',
  'settings.delegate.submitCode',
  'settings.autonomy.setMode',
  'settings.delegate.setDenyRules',
]

function sample(schema: JsonSchema): unknown {
  if (schema.enum) return schema.enum[0]
  switch (schema.type) {
    case 'string':
      return 'x'
    case 'integer':
    case 'number':
      return 1
    case 'boolean':
      return true
    case 'array':
      return []
    case 'object': {
      const value: Record<string, unknown> = {}
      for (const key of schema.required ?? []) {
        const child = schema.properties?.[key]
        if (child) value[key] = sample(child)
      }
      return value
    }
  }
}

function catalogRunner(calls: string[]) {
  const record: ActionHandler = () => {
    calls.push('handled')
    return { done: true }
  }
  return createActionRunner({
    catalog: ALL_ACTIONS,
    globalHandler: () => record,
    resolveFocus: () => 'focused',
    targetExists: () => true,
    awaitTabHandler: async () => record,
  })
}

test('the guardrail scope contains every expected user-only action', () => {
  const guardrails = ALL_ACTIONS.filter((a) => a.scope === 'guardrails').map(
    (a) => a.id,
  )
  for (const id of EXPECTED_GUARDRAILS) assert.ok(guardrails.includes(id), id)
})

test('every guardrail action is refused for both agent kinds and runs for the user (SC-008)', async () => {
  for (const action of ALL_ACTIONS.filter((a) => a.scope === 'guardrails')) {
    const input = sample(action.input) as Record<string, unknown>
    for (const caller of [BUILTIN, EXTERNAL]) {
      const calls: string[] = []
      const outcome = await catalogRunner(calls).runAction(
        action.id,
        input,
        caller,
      )
      assert.equal(
        !outcome.ok && outcome.code,
        'forbidden_for_agents',
        action.id,
      )
      assert.equal(calls.length, 0, action.id)
    }
    const calls: string[] = []
    const outcome = await catalogRunner(calls).runAction(action.id, input, USER)
    assert.equal(outcome.ok, true, action.id)
    assert.equal(calls.length, 1, action.id)
  }
})

test('every other action is callable by an agent naming its target, except the platform back hook', async () => {
  const field = { tab: 'tabId', window: 'windowId', workspace: 'workspaceId' }
  for (const action of ALL_ACTIONS.filter((a) => a.scope !== 'guardrails')) {
    if (action.id === 'shell.system.back') {
      assert.equal(action.agentCallable, false)
      continue
    }
    assert.equal(action.agentCallable, true, action.id)
    const input = sample(action.input) as Record<string, unknown>
    if (action.target !== 'none') input[field[action.target]] = 'target'
    const outcome = await catalogRunner([]).runAction(
      action.id,
      input,
      EXTERNAL,
    )
    assert.equal(outcome.ok, true, action.id)
  }
})

test('the catalog offers the read actions agents observe with (FR-028)', () => {
  const reads = new Set(
    ALL_ACTIONS.filter((a) => a.effect === 'read').map((a) => a.id),
  )
  for (const id of [
    'shell.state.get',
    'shell.tab.history',
    'shell.apps.list',
    'shell.actions.list',
    'chat.conversations.list',
    'chat.messages.list',
    'settings.get',
  ])
    assert.ok(reads.has(id), id)
})
