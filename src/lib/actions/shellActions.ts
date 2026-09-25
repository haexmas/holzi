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
    yieldToTextInput: { mac: true },
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
    yieldToTextInput: { mac: true },
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
]
