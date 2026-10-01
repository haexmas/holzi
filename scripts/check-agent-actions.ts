// `pnpm check:agent-actions` (spec 032-model-operates-holzi): the action → tool mapping for the
// built-in agent under `src/lib/actions/agentTools.ts` and the invariants over the shipped catalog
// that keep the model's offer safe — tool names, the guardrail lock, the actions withheld from the
// built-in agent, the core offer, and that every read action runs for a built-in agent caller.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  CORE_AGENT_TOOLS,
  agentSafeMessages,
  describeAction,
  describeToolCall,
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
import type { ActionCaller, JsonSchema } from '../src/lib/actions/types.ts'
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

test('every guardrail action is refused for the built-in agent before its handler runs (FR-003, SC-003)', async () => {
  const guardrails = ALL_ACTIONS.filter((a) => a.scope === 'guardrails')
  assert.ok(guardrails.length > 0)
  for (const action of guardrails) {
    const input = sample(action.input) as Record<string, unknown>
    if (action.target !== 'none') input[TARGET_FIELD[action.target]] = 'target'
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
      assert.equal(calls.length, 0, `${action.id} ran`)
    }
  }
})

const SECRET_NAME =
  /secret|private|password|passphrase|token|apiKey|credential/i

/** Names allowed to match, each with the reason it is no secret. Keep this short. */
const SECRET_NAME_ALLOWED: Record<string, string> = {
  tokenizerRepo:
    'a Hugging Face repository id of a tokenizer, not a credential',
}

function propertyNames(schema: JsonSchema): string[] {
  return [
    ...Object.entries(schema.properties ?? {}).flatMap(([name, child]) => [
      name,
      ...propertyNames(child),
    ]),
    ...(schema.items ? propertyNames(schema.items) : []),
  ]
}

test('no input or result schema of an agent-callable action names a secret (FR-010, SC-004)', () => {
  for (const action of ALL_ACTIONS.filter(isBuiltinAgentAction)) {
    for (const [kind, schema] of [
      ['input', action.input],
      ['result', action.result],
    ] as const) {
      for (const name of propertyNames(schema)) {
        if (name in SECRET_NAME_ALLOWED) continue
        assert.doesNotMatch(
          name,
          SECRET_NAME,
          `${action.id} ${kind} has a field named ${name}`,
        )
      }
    }
  }
})

test('settings.devices.identity exposes the public key and nothing else', () => {
  const action = ALL_ACTIONS.find((a) => a.id === 'settings.devices.identity')
  assert.ok(action && isBuiltinAgentAction(action))
  assert.deepEqual(Object.keys(action.result.properties ?? {}).sort(), [
    'hex',
    'npub',
  ])
})

test('chat.messages.list shows an agent no text, neither as content nor inside another field (SC-004)', () => {
  const secret = 'sk-ant-api03-this-is-a-secret'
  const rows = [
    {
      id: 'm1',
      role: 'user',
      content: `my key is ${secret}`,
      createdAt: 1,
      finishReason: null,
      toolInput: JSON.stringify({ key: secret }),
    },
    {
      id: 'm2',
      role: 'assistant',
      content: 'ok',
      createdAt: 2,
      finishReason: 'complete',
      toolInput: null,
    },
  ]
  const projected = agentSafeMessages(rows)
  assert.deepEqual(projected, {
    messages: [
      { id: 'm1', role: 'user', createdAt: 1, status: null },
      { id: 'm2', role: 'assistant', createdAt: 2, status: 'complete' },
    ],
  })
  assert.ok(!JSON.stringify(projected).includes(secret))
  assert.ok(!JSON.stringify(projected).includes('content'))
})

test('describeAction names the action, its target and its inputs for the approval dialog (FR-008)', () => {
  const close = ALL_ACTIONS.find((a) => a.id === 'wm.tab.close')
  assert.ok(close)
  const resolve = (kind: string, id: string) =>
    kind === 'tab' && id === 't1' ? 'Settings' : undefined
  const known = describeAction(close, { tabId: 't1' }, resolve)
  assert.equal(known.titleKey, close.titleKey)
  assert.deepEqual(known.target, { kind: 'tab', label: 'Settings' })
  assert.deepEqual(known.inputs, [])
  // A target that does not exist keeps its id as the label rather than hiding it.
  assert.deepEqual(describeAction(close, { tabId: 'gone' }, resolve).target, {
    kind: 'tab',
    label: 'gone',
  })
  // No target given: none is shown.
  assert.equal(describeAction(close, {}, resolve).target, undefined)
  // Other inputs become fields; long values are cut.
  const rename = ALL_ACTIONS.find((a) => a.id === 'chat.conversation.rename')
  assert.ok(rename)
  const long = describeAction(
    rename,
    { threadId: 'x', title: 'a'.repeat(300) },
    resolve,
  )
  assert.equal(long.inputs.length, 2)
  assert.equal(long.inputs[1]?.[1].length, 201)
})

test('describeToolCall falls back for a name that is no offered action', () => {
  const resolve = () => undefined
  assert.equal(
    describeToolCall('run_command', {}, ALL_ACTIONS, resolve),
    undefined,
  )
  assert.equal(
    describeToolCall('find_actions', {}, ALL_ACTIONS, resolve),
    undefined,
  )
  assert.equal(
    describeToolCall('chat_message_send', { text: 'x' }, ALL_ACTIONS, resolve),
    undefined,
  )
  assert.ok(describeToolCall('settings_get', null, ALL_ACTIONS, resolve))
})
