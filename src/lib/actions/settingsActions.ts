// Settings action catalog (spec 020-tab-navigation, T051, contracts/wm-actions.md §1). All
// global: they work without the settings window (research R19); handlers in
// `stores/settingsActionHandlers.ts`. Provider connections, the autonomy mode and the delegate
// deny rules are guardrails — never callable by agents (FR-032). `settings.get` never returns
// provider credentials.
import type { JsonSchema, ActionDefinition } from './types.ts'

const DONE: JsonSchema = {
  type: 'object',
  properties: { done: { type: 'boolean' } },
}
const ANY_OBJECT: JsonSchema = { type: 'object' }
const NO_INPUT: JsonSchema = { type: 'object', properties: {} }
const SCOPE: JsonSchema = {
  type: 'string',
  enum: ['device', 'vault'],
  description: "'device' applies to this device only, 'vault' to all devices.",
}
const VENDOR: JsonSchema = { type: 'string', enum: ['claude', 'codex'] }
const RESTORE_STATE: JsonSchema = {
  type: 'object',
  description:
    "Session restore setting: 'device' and 'vault' are true, false or null (unset); 'effective' is what applies on this device.",
  properties: {
    device: { type: 'boolean' },
    vault: { type: 'boolean' },
    effective: { type: 'boolean' },
  },
}
const MODEL_ID: JsonSchema = {
  type: 'string',
  description: 'Installed model id (see settings.models.list).',
}

type Spec = Pick<ActionDefinition, 'id' | 'description' | 'scope' | 'effect'> &
  Partial<Pick<ActionDefinition, 'input' | 'result'>>

function setting(spec: Spec): ActionDefinition {
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

export const SETTINGS_ACTIONS: readonly ActionDefinition[] = [
  setting({
    id: 'settings.get',
    description:
      'Read the current settings: device name, default and speech models, session restore, autonomy mode and delegate deny rules. Never includes credentials.',
    result: ANY_OBJECT,
    scope: 'settings.read',
    effect: 'read',
  }),
  setting({
    id: 'settings.models.list',
    description: 'List the installed models.',
    result: ANY_OBJECT,
    scope: 'settings.read',
    effect: 'read',
  }),
  setting({
    id: 'settings.models.checkUpdates',
    description: 'Check installed HuggingFace models for newer revisions.',
    result: ANY_OBJECT,
    scope: 'settings.read',
    effect: 'read',
  }),
  setting({
    id: 'settings.device.setAlias',
    description: 'Rename this device (the name other devices see).',
    input: {
      type: 'object',
      properties: { alias: { type: 'string' } },
      required: ['alias'],
    },
    scope: 'settings.device',
    effect: 'write',
  }),
  setting({
    id: 'settings.sessionRestore.set',
    description:
      'Turn saving and restoring the open workspaces, windows and tabs (with their back/forward history) on or off, for this device or for the whole vault. Turning it off deletes the saved session.',
    input: {
      type: 'object',
      properties: { scope: SCOPE, enabled: { type: 'boolean' } },
      required: ['scope', 'enabled'],
    },
    result: RESTORE_STATE,
    scope: 'settings.device',
    effect: 'write',
  }),
  setting({
    id: 'settings.sessionRestore.clear',
    description:
      'Reset the session restore value for this device or the vault, so the other value applies (or off if neither is set).',
    input: {
      type: 'object',
      properties: { scope: SCOPE },
      required: ['scope'],
    },
    result: RESTORE_STATE,
    scope: 'settings.device',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.setDefault',
    description:
      'Set the default chat model for this device or the whole vault.',
    input: {
      type: 'object',
      properties: { modelId: MODEL_ID, scope: SCOPE },
      required: ['modelId', 'scope'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.clearDefault',
    description: 'Remove the default chat model for this device or the vault.',
    input: {
      type: 'object',
      properties: { scope: SCOPE },
      required: ['scope'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.setStt',
    description:
      'Switch the speech-to-text model to a catalog entry, downloading it if needed.',
    input: {
      type: 'object',
      properties: { catalogId: { type: 'string' } },
      required: ['catalogId'],
    },
    result: {
      type: 'object',
      properties: { modelId: { type: 'string' } },
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.downloadCatalog',
    description: 'Download a chat model from the recommended catalog.',
    input: {
      type: 'object',
      properties: { entryId: { type: 'string' } },
      required: ['entryId'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.downloadFromHf',
    description:
      'Download a GGUF file from a HuggingFace repository as a model.',
    input: {
      type: 'object',
      properties: {
        repoId: { type: 'string' },
        filename: { type: 'string' },
        revision: { type: 'string' },
        name: { type: 'string' },
        tokenizerRepo: { type: 'string' },
        contextWindow: { type: 'integer' },
        forceTooBig: { type: 'boolean' },
      },
      required: ['repoId', 'filename'],
    },
    result: ANY_OBJECT,
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.installUpdate',
    description:
      'Install the newer revision of an installed HuggingFace model.',
    input: {
      type: 'object',
      properties: { modelId: MODEL_ID },
      required: ['modelId'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.delete',
    description: 'Delete an installed model from disk.',
    input: {
      type: 'object',
      properties: { modelId: MODEL_ID },
      required: ['modelId'],
    },
    scope: 'settings.models',
    effect: 'destructive',
  }),
  setting({
    id: 'settings.delegate.refreshModels',
    description: "Refresh a connected delegate provider's model list.",
    input: {
      type: 'object',
      properties: { providerId: { type: 'string' } },
      required: ['providerId'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.delegate.connectProvider',
    description: 'Start connecting a CLI delegate provider. User only.',
    input: {
      type: 'object',
      properties: { vendor: VENDOR, name: { type: 'string' } },
      required: ['vendor', 'name'],
    },
    result: {
      type: 'object',
      properties: { status: { type: 'string' } },
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  setting({
    id: 'settings.delegate.submitCode',
    description:
      'Submit the sign-in code of a pending delegate provider connection. User only.',
    input: {
      type: 'object',
      properties: { code: { type: 'string' }, name: { type: 'string' } },
      required: ['code', 'name'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  setting({
    id: 'settings.autonomy.setMode',
    description: "Set this device's delegate autonomy mode. User only.",
    input: {
      type: 'object',
      properties: {
        mode: {
          type: 'string',
          enum: ['standard', 'ungated', 'gated_permissive'],
        },
      },
      required: ['mode'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  setting({
    id: 'settings.delegate.setDenyRules',
    description: "Set this device's delegate deny rules. User only.",
    input: {
      type: 'object',
      properties: { rules: { type: 'array', items: { type: 'string' } } },
      required: ['rules'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
]
