// Chat action catalog (spec 020-tab-navigation, T049, contracts/shell-actions.md §1). Tab-bound
// actions run in the mounted chat (`composables/useChatShell.ts` registers them; the runner opens
// the chat first if needed). Model, reasoning and voice preferences live in global stores and are
// global actions (`stores/chatActionHandlers.ts`). Approvals, the permission mode and model
// integrity overrides are guardrails: never callable by agents (FR-032).
//
// Exempt from the catalog (`action-exempt:` at the control): draft editing (typing, attachments,
// voice recording as an input method) and view state (dialogs, accordions, editing mode, error
// dismissal, HuggingFace browsing and previews).
import type { JsonSchema, ShellActionDefinition } from './types.ts'

const THREAD_ID: JsonSchema = {
  type: 'string',
  description: 'Conversation id (see chat.conversations.list).',
}
const DONE: JsonSchema = {
  type: 'object',
  properties: { done: { type: 'boolean' } },
}
const ANY_OBJECT: JsonSchema = { type: 'object' }
const NO_INPUT: JsonSchema = { type: 'object', properties: {} }

type Spec = Pick<
  ShellActionDefinition,
  'id' | 'description' | 'scope' | 'effect'
> &
  Partial<Pick<ShellActionDefinition, 'input' | 'result' | 'agentCallable'>>

function inChat(spec: Spec): ShellActionDefinition {
  return {
    titleKey: `actions.${spec.id}`,
    input: NO_INPUT,
    result: DONE,
    target: 'none',
    agentCallable: spec.scope !== 'guardrails',
    binding: 'tab',
    appId: 'system.chat',
    ...spec,
  }
}

function global(spec: Spec): ShellActionDefinition {
  return {
    titleKey: `actions.${spec.id}`,
    input: NO_INPUT,
    result: DONE,
    target: 'none',
    agentCallable: spec.scope !== 'guardrails',
    binding: 'global',
    ...spec,
  }
}

export const CHAT_ACTIONS: readonly ShellActionDefinition[] = [
  inChat({
    id: 'chat.conversation.new',
    description: 'Start a new conversation in the chat.',
    scope: 'chat.write',
    effect: 'write',
  }),
  inChat({
    id: 'chat.conversation.open',
    description: 'Open an existing conversation in the chat.',
    input: {
      type: 'object',
      properties: { threadId: THREAD_ID },
      required: ['threadId'],
    },
    scope: 'chat.read',
    effect: 'write',
  }),
  inChat({
    id: 'chat.conversation.rename',
    description: 'Rename a conversation.',
    input: {
      type: 'object',
      properties: { threadId: THREAD_ID, title: { type: 'string' } },
      required: ['threadId', 'title'],
    },
    scope: 'chat.write',
    effect: 'write',
  }),
  inChat({
    id: 'chat.conversation.delete',
    description:
      'Delete a conversation with all its messages; a running reply in it is cancelled first.',
    input: {
      type: 'object',
      properties: { threadId: THREAD_ID },
      required: ['threadId'],
    },
    scope: 'chat.write',
    effect: 'destructive',
  }),
  inChat({
    id: 'chat.conversations.list',
    description: 'List all conversations with id, title and creation time.',
    result: ANY_OBJECT,
    scope: 'chat.read',
    effect: 'read',
  }),
  inChat({
    id: 'chat.messages.list',
    description: 'List the messages of a conversation.',
    input: {
      type: 'object',
      properties: { threadId: THREAD_ID },
      required: ['threadId'],
    },
    result: ANY_OBJECT,
    scope: 'chat.read',
    effect: 'read',
  }),
  inChat({
    id: 'chat.message.send',
    description:
      'Send a message in the open conversation (a new conversation if none is open) and start a reply.',
    input: {
      type: 'object',
      properties: { text: { type: 'string' } },
      required: ['text'],
    },
    scope: 'chat.write',
    effect: 'write',
  }),
  inChat({
    id: 'chat.message.retry',
    description: 'Retry the last message that failed to send.',
    scope: 'chat.write',
    effect: 'write',
  }),
  inChat({
    id: 'chat.reply.cancel',
    description: 'Cancel the running reply.',
    scope: 'chat.write',
    effect: 'write',
  }),
  inChat({
    id: 'chat.approval.decide',
    description: 'Allow or deny a pending tool permission request. User only.',
    input: {
      type: 'object',
      properties: {
        requestId: { type: 'string' },
        decision: { type: 'string', enum: ['allow', 'deny'] },
      },
      required: ['requestId', 'decision'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  inChat({
    id: 'chat.permissionMode.set',
    description:
      'Set the tool permission mode (manual, auto, plan). User only.',
    input: {
      type: 'object',
      properties: {
        mode: { type: 'string', enum: ['manual', 'auto', 'plan'] },
      },
      required: ['mode'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
]

export const CHAT_MODEL_ACTIONS: readonly ShellActionDefinition[] = [
  global({
    id: 'chat.model.select',
    description: 'Choose and load the model the chat answers with.',
    input: {
      type: 'object',
      properties: { modelId: { type: 'string' } },
      required: ['modelId'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  global({
    id: 'chat.reasoning.set',
    description:
      "Set the reasoning effort for the active model; omit 'level' for automatic.",
    input: {
      type: 'object',
      properties: { level: { type: 'string' } },
    },
    scope: 'chat.write',
    effect: 'write',
  }),
  global({
    id: 'chat.model.retryLoad',
    description: 'Retry loading the active model after a failure.',
    scope: 'settings.models',
    effect: 'write',
  }),
  global({
    id: 'chat.model.downloadRecommended',
    description: 'Download a model from the recommended catalog.',
    input: {
      type: 'object',
      properties: { entryId: { type: 'string' } },
      required: ['entryId'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  global({
    id: 'chat.modelIntegrity.decide',
    description:
      'Resolve a failed model integrity check: load anyway, repair the source, or choose another model. User only.',
    input: {
      type: 'object',
      properties: {
        decision: {
          type: 'string',
          enum: ['loadUntrusted', 'repairSource', 'chooseOther'],
        },
      },
      required: ['decision'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  global({
    id: 'chat.voice.setAutoSend',
    description:
      'Send dictated text automatically when recording stops, or not.',
    input: {
      type: 'object',
      properties: { enabled: { type: 'boolean' } },
      required: ['enabled'],
    },
    scope: 'chat.write',
    effect: 'write',
  }),
]
