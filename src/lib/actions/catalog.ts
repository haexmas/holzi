// The complete action catalog (spec 020-tab-navigation): every shipped action definition, in one
// list, for the runner, `shell.actions.list` and the catalog invariants in check-shell-actions.ts.
// The per-area catalogs (shell, chat, settings) are added here as their stories land.
import { SHELL_NAVIGATION_ACTIONS, SHELL_OPEN_ACTIONS } from './shellActions.ts'
import {
  SHELL_LAYOUT_ACTIONS,
  SHELL_READ_ACTIONS,
} from './shellLayoutActions.ts'
import { CHAT_ACTIONS, CHAT_MODEL_ACTIONS } from './chatActions.ts'
import type { ShellActionDefinition } from './types.ts'

export const ALL_ACTIONS: readonly ShellActionDefinition[] = [
  ...SHELL_NAVIGATION_ACTIONS,
  ...SHELL_OPEN_ACTIONS,
  ...SHELL_LAYOUT_ACTIONS,
  ...SHELL_READ_ACTIONS,
  ...CHAT_ACTIONS,
  ...CHAT_MODEL_ACTIONS,
]
