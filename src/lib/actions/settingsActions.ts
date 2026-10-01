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
const VENDOR: JsonSchema = { type: 'string', enum: ['claude', 'codex'] }
const RESTORE_STATE: JsonSchema = {
  type: 'object',
  description:
    'Session restore setting of the vault: whether open workspaces, windows and tabs are saved and restored on every device.',
  properties: { enabled: { type: 'boolean' } },
  required: ['enabled'],
}
const COLOR_SCHEME: JsonSchema = {
  type: 'string',
  enum: ['light', 'dark', 'system'],
}
const COLOR_SCHEME_STATE: JsonSchema = {
  type: 'object',
  description:
    "Color scheme of the vault ('system' follows the operating system).",
  properties: { scheme: COLOR_SCHEME },
  required: ['scheme'],
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
      'Read the current settings: device name, color scheme, default and speech models, session restore, autonomy mode and delegate deny rules. Never includes credentials.',
    result: ANY_OBJECT,
    scope: 'settings.read',
    effect: 'read',
  }),
  setting({
    id: 'settings.devices.list',
    description:
      "List the vault's devices: this device first and marked, then the others by name, each with its role (main or linked), whether it is online, when it was last online (epoch milliseconds, absent if never seen) and why sync with it is halted, if it is. A device without a name has no alias.",
    result: ANY_OBJECT,
    scope: 'settings.read',
    effect: 'read',
  }),
  setting({
    id: 'settings.devices.identity',
    description:
      "The vault's public identity: its public key as npub and hex, which names the vault in member lists and rights. Never a private key.",
    result: {
      type: 'object',
      properties: { npub: { type: 'string' }, hex: { type: 'string' } },
      required: ['npub', 'hex'],
    },
    scope: 'settings.read',
    effect: 'read',
  }),
  setting({
    // Changes where devices find each other: a guardrail, never callable by agents (FR-036).
    id: 'settings.sync.servers.set',
    description:
      'Set the Nostr and iroh servers devices find each other through: the ones added besides the built-in public servers, and the ones (built-in or added) switched off.',
    input: {
      type: 'object',
      properties: {
        nostrRelays: { type: 'array', items: { type: 'string' } },
        irohRelays: { type: 'array', items: { type: 'string' } },
        disabled: { type: 'array', items: { type: 'string' } },
      },
      required: ['nostrRelays', 'irohRelays', 'disabled'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  setting({
    // Locks a device out of the vault: a guardrail, never callable by agents (FR-036).
    id: 'settings.devices.remove',
    description:
      'Remove a device from the vault (main devices only, never the device itself). It gets no new data and cannot read new changes; what is on it stays.',
    input: {
      type: 'object',
      properties: { devicePubkey: { type: 'string' } },
      required: ['devicePubkey'],
    },
    scope: 'guardrails',
    effect: 'destructive',
  }),
  setting({
    // Lets a copy of the vault file into the vault (or keeps it out): a guardrail, never callable
    // by agents (spec 024, FR-036, FR-045).
    id: 'settings.devices.admit',
    description:
      'Admit or refuse the request of a copy of the vault file to join the vault (main devices only).',
    input: {
      type: 'object',
      properties: {
        devicePubkey: { type: 'string' },
        admit: { type: 'boolean' },
      },
      required: ['devicePubkey', 'admit'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
  setting({
    // Opens the view that shows a link code. Linking adds a device to the vault, so it is a
    // guardrail: never callable by agents (spec 024, FR-036).
    id: 'settings.devices.link',
    description:
      'Open the settings view that links a new device to this vault with a code (main devices only).',
    scope: 'guardrails',
    effect: 'write',
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
      'Turn saving and restoring the open workspaces, windows and tabs (with their back/forward history) on or off for the whole vault. Turning it off deletes the saved session.',
    input: {
      type: 'object',
      properties: { enabled: { type: 'boolean' } },
      required: ['enabled'],
    },
    result: RESTORE_STATE,
    scope: 'settings.device',
    effect: 'write',
  }),
  setting({
    id: 'settings.appearance.setColorScheme',
    description:
      'Set the color scheme (light, dark or following the system) for the whole vault. It applies at once.',
    input: {
      type: 'object',
      properties: { scheme: COLOR_SCHEME },
      required: ['scheme'],
    },
    result: COLOR_SCHEME_STATE,
    scope: 'settings.device',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.setDefault',
    description:
      'Set the default chat model of this device (models are installed per device).',
    input: {
      type: 'object',
      properties: { modelId: MODEL_ID },
      required: ['modelId'],
    },
    scope: 'settings.models',
    effect: 'write',
  }),
  setting({
    id: 'settings.models.clearDefault',
    description: 'Remove the default chat model of this device.',
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
    description: 'Set the delegate autonomy mode of the vault. User only.',
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
    description: 'Set the delegate deny rules of the vault. User only.',
    input: {
      type: 'object',
      properties: { rules: { type: 'array', items: { type: 'string' } } },
      required: ['rules'],
    },
    scope: 'guardrails',
    effect: 'write',
  }),
]
