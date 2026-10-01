// `pnpm check:agent-actions` (spec 032-model-operates-holzi): the action → tool mapping for the
// built-in agent under `src/lib/actions/agentTools.ts` and the invariants over the shipped catalog
// that keep the model's offer safe — tool names, the guardrail lock, the actions withheld from the
// built-in agent, the core offer, and that every read action runs for a built-in agent caller.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  CORE_AGENT_TOOLS,
  TOOL_NAME_PATTERN,
  fromToolName,
  isBuiltinAgentAction,
  listAgentActions,
  toAgentActionDef,
  toOutcomeWire,
  toToolName,
  type TitleOf,
} from '../src/lib/actions/agentTools.ts'
import { ALL_ACTIONS } from '../src/lib/actions/catalog.ts'
import type { ActionCaller } from '../src/lib/actions/types.ts'
import { TARGET_FIELD, catalogRunner, sample } from './lib/actions-harness.ts'

const BUILTIN: ActionCaller = { kind: 'builtinAgent' }
const EXTERNAL: ActionCaller = { kind: 'externalAgent', agentId: 'agent-1' }

/** Withheld from the built-in agent: they would re-enter the running chat turn (ADR-0006). */
const WITHHELD_FROM_BUILTIN = [
  'chat.message.send',
  'chat.message.retry',
  'chat.reply.cancel',
]

const titleOf: TitleOf = (key, locale) => `${locale}:${key}`

test('tool names match the provider pattern and are unique over the whole catalog', () => {
  const names = ALL_ACTIONS.map((a) => toToolName(a.id))
  for (const name of names) assert.match(name, TOOL_NAME_PATTERN, name)
  assert.equal(new Set(names).size, names.length)
})

test('toToolName is reversible over the catalog', () => {
  for (const action of ALL_ACTIONS)
    assert.equal(fromToolName(toToolName(action.id), ALL_ACTIONS), action)
  assert.equal(fromToolName('run_command', ALL_ACTIONS), undefined)
})

test('the built-in agent is offered no guardrail action (FR-003)', () => {
  const offered = listAgentActions(ALL_ACTIONS, titleOf)
  const guardrails = ALL_ACTIONS.filter((a) => a.scope === 'guardrails')
  assert.ok(guardrails.length > 0)
  for (const action of guardrails) {
    assert.ok(
      !offered.some((o) => o.actionId === action.id),
      `${action.id} is offered`,
    )
  }
  for (const def of offered) {
    const action = ALL_ACTIONS.find((a) => a.id === def.actionId)
    assert.ok(action && isBuiltinAgentAction(action), def.actionId)
  }
})

test('exactly the three self-triggering chat actions are withheld from the built-in agent', () => {
  const withheld = ALL_ACTIONS.filter(
    (a) => a.builtinAgentCallable === false,
  ).map((a) => a.id)
  assert.deepEqual(withheld.sort(), [...WITHHELD_FROM_BUILTIN].sort())
  const offered = listAgentActions(ALL_ACTIONS, titleOf).map((d) => d.actionId)
  for (const id of WITHHELD_FROM_BUILTIN) {
    assert.ok(!offered.includes(id), id)
    const action = ALL_ACTIONS.find((a) => a.id === id)
    assert.equal(action?.agentCallable, true, `${id} stays open to spec 021`)
  }
})

test('withheld actions are refused for the built-in agent and accepted for external agents', async () => {
  for (const id of WITHHELD_FROM_BUILTIN) {
    const action = ALL_ACTIONS.find((a) => a.id === id)
    assert.ok(action)
    const input = sample(action.input) as Record<string, unknown>
    const calls: string[] = []
    const refused = await catalogRunner(calls).runAction(id, input, BUILTIN)
    assert.equal(!refused.ok && refused.code, 'forbidden_for_agents', id)
    assert.equal(calls.length, 0, id)
    const accepted = await catalogRunner(calls).runAction(id, input, EXTERNAL)
    assert.equal(accepted.ok, true, id)
  }
})

test('toAgentActionDef passes the action through unchanged and adds localized titles', () => {
  const action = ALL_ACTIONS.find((a) => a.id === 'wm.app.open')
  assert.ok(action)
  const def = toAgentActionDef(action, titleOf)
  assert.equal(def.toolName, 'wm_app_open')
  assert.equal(def.actionId, 'wm.app.open')
  assert.equal(def.description, action.description)
  assert.equal(def.inputSchema, action.input)
  assert.equal(def.effect, action.effect)
  assert.deepEqual(def.titles, {
    de: `de:${action.titleKey}`,
    en: `en:${action.titleKey}`,
  })
})

test('the core offer names existing, built-in-callable actions and leaves room for find_actions', () => {
  assert.ok(
    CORE_AGENT_TOOLS.length <= 9,
    'core plus find_actions is at most 10',
  )
  assert.equal(new Set(CORE_AGENT_TOOLS).size, CORE_AGENT_TOOLS.length)
  for (const id of CORE_AGENT_TOOLS) {
    const action = ALL_ACTIONS.find((a) => a.id === id)
    assert.ok(action, `${id} is not in the catalog`)
    assert.ok(isBuiltinAgentAction(action), id)
  }
  const defs = listAgentActions(ALL_ACTIONS, titleOf)
  const core = defs.filter((d) => d.core).map((d) => d.actionId)
  assert.deepEqual(core.sort(), [...CORE_AGENT_TOOLS].sort())
})

test('every built-in-callable read action runs for a built-in agent naming its target', async () => {
  const reads = ALL_ACTIONS.filter(
    (a) => a.effect === 'read' && isBuiltinAgentAction(a),
  )
  assert.ok(reads.length > 0)
  for (const action of reads) {
    const input = sample(action.input) as Record<string, unknown>
    if (action.target !== 'none') input[TARGET_FIELD[action.target]] = 'target'
    const outcome = await catalogRunner([]).runAction(action.id, input, BUILTIN)
    assert.equal(outcome.ok, true, action.id)
  }
})

test('wm.state.get tells the model which ids serve as action targets (FR-004)', () => {
  const action = ALL_ACTIONS.find((a) => a.id === 'wm.state.get')
  assert.ok(action && isBuiltinAgentAction(action))
  for (const field of ['workspaceId', 'windowId', 'tabId'])
    assert.ok(action.description.includes(field), field)
})

test('toOutcomeWire never carries the raw error of a failed handler (FR-006)', () => {
  const secret = new Error('cannot read /home/someone/vault.db')
  const failed = toOutcomeWire({
    ok: false,
    code: 'failed',
    message: secret.message,
    error: secret,
  })
  assert.deepEqual(Object.keys(failed).sort(), ['code', 'message', 'ok'])
  const invalid = toOutcomeWire({
    ok: false,
    code: 'invalid_input',
    message: 'bad',
    field: 'text',
  })
  assert.deepEqual(invalid, {
    ok: false,
    code: 'invalid_input',
    field: 'text',
    message: 'bad',
  })
  assert.deepEqual(toOutcomeWire({ ok: true, result: { done: true } }), {
    ok: true,
    result: { done: true },
  })
})
