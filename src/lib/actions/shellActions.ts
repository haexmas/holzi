// Shell action catalog (spec 020-tab-navigation, contracts/shell-actions.md §1). Pure data; the
// handlers are registered by `stores/shellActionHandlers.ts`. Descriptions are English because they
// become tool descriptions for agents (spec 021).
import type { JsonSchema, ShellActionDefinition } from './types.ts'

const TAB_ID: JsonSchema = {
  type: 'string',
  description:
    'Tab to act on. Required for agents; users default to the focused tab.',
}

const MOVED: JsonSchema = {
  type: 'object',
  properties: {
    moved: {
      type: 'boolean',
      description: 'Whether the tab changed location.',
    },
  },
}

/** Back/forward, history jumps and in-tab navigation (FR-004–FR-007, FR-017). */
export const SHELL_NAVIGATION_ACTIONS: readonly ShellActionDefinition[] = [
  {
    id: 'shell.tab.back',
    titleKey: 'actions.shell.tab.back',
    description: "Go back one entry in a tab's own history.",
    input: { type: 'object', properties: { tabId: TAB_ID } },
    result: MOVED,
    target: 'tab',
    scope: 'shell.navigation',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
    defaultKeys: {
      default: ['Alt+ArrowLeft'],
      mac: ['Alt+ArrowLeft', 'Meta+BracketLeft'],
    },
    yieldToTextInput: { mac: ['Alt+ArrowLeft'] },
  },
  {
    id: 'shell.tab.forward',
    titleKey: 'actions.shell.tab.forward',
    description: "Go forward one entry in a tab's own history.",
    input: { type: 'object', properties: { tabId: TAB_ID } },
    result: MOVED,
    target: 'tab',
    scope: 'shell.navigation',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
    defaultKeys: {
      default: ['Alt+ArrowRight'],
      mac: ['Alt+ArrowRight', 'Meta+BracketRight'],
    },
    yieldToTextInput: { mac: ['Alt+ArrowRight'] },
  },
  {
    id: 'shell.tab.go',
    titleKey: 'actions.shell.tab.go',
    description:
      "Jump several entries in a tab's history: negative steps go back, positive steps go forward.",
    input: {
      type: 'object',
      properties: {
        tabId: TAB_ID,
        steps: { type: 'integer', description: 'Signed number of entries.' },
      },
      required: ['steps'],
    },
    result: MOVED,
    target: 'tab',
    scope: 'shell.navigation',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
  },
  {
    id: 'shell.tab.navigate',
    titleKey: 'actions.shell.tab.navigate',
    description:
      "Navigate a tab to a location inside its app, e.g. '/thread/<id>'. replace=true changes the current entry instead of adding one.",
    input: {
      type: 'object',
      properties: {
        tabId: TAB_ID,
        to: {
          type: 'string',
          description:
            "App-relative location starting with '/', optionally with a query.",
        },
        replace: { type: 'boolean' },
      },
      required: ['to'],
    },
    result: MOVED,
    target: 'tab',
    scope: 'shell.navigation',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
  },
  {
    id: 'shell.system.back',
    titleKey: 'actions.shell.system.back',
    description:
      'Platform back (Android back gesture): close a shell overlay, else go back in the top visible tab, else open the window overview in compact mode.',
    input: { type: 'object', properties: {} },
    result: {
      type: 'object',
      properties: {
        outcome: {
          type: 'string',
          enum: ['closeOverlay', 'back', 'openWindowOverview', 'none'],
        },
      },
    },
    target: 'none',
    scope: 'shell.navigation',
    effect: 'write',
    agentCallable: false,
    binding: 'global',
  },
]

const APP_ID: JsonSchema = {
  type: 'string',
  description:
    "App id, e.g. 'system.chat', 'system.settings' (see shell.apps.list).",
}
const AT: JsonSchema = {
  type: 'string',
  description: "Optional app-relative start location, e.g. '/thread/<id>'.",
}
const OPENED: JsonSchema = {
  type: 'object',
  properties: {
    tabId: { type: 'string', description: 'The tab that shows the app.' },
    created: {
      type: 'boolean',
      description:
        'false when an already open single-instance app was activated instead.',
    },
  },
}

/** Opening apps and tabs at a location (FR-012). */
export const SHELL_OPEN_ACTIONS: readonly ShellActionDefinition[] = [
  {
    id: 'shell.app.open',
    titleKey: 'actions.shell.app.open',
    description:
      'Open an app in a new window, optionally at a location. A single-instance app that is already open is activated and navigated there instead.',
    input: {
      type: 'object',
      properties: { appId: APP_ID, at: AT },
      required: ['appId'],
    },
    result: OPENED,
    target: 'none',
    scope: 'shell.navigation',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
  },
  {
    id: 'shell.tab.new',
    titleKey: 'actions.shell.tab.new',
    description:
      'Open an app as a new tab in a window, optionally at a location. A single-instance app that is already open is activated instead.',
    input: {
      type: 'object',
      properties: {
        windowId: {
          type: 'string',
          description:
            'Window to add the tab to. Required for agents; users default to the focused window.',
        },
        appId: APP_ID,
        at: AT,
      },
      required: ['appId'],
    },
    result: OPENED,
    target: 'window',
    scope: 'shell.layout',
    effect: 'write',
    agentCallable: true,
    binding: 'global',
  },
]
