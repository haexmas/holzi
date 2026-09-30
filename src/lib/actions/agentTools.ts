// Actions as the built-in agent's tools (spec 032, data-model.md §2, ADR-0006). The catalog stays the
// single source of the definitions; this module maps an action to the tool description the Rust
// `ToolRegistry` keeps (`set_agent_actions`). Pure TS, relative `.ts` imports only, no i18n: titles
// come in through `titleOf` so the Node check scripts can load the module.
import type { ActionDefinition, ActionOutcome, JsonSchema } from './types.ts'

export type ActionLocale = 'de' | 'en'

/** What the frontend pushes to Rust for one action (data-model.md §2). */
export type AgentActionDef = {
  /** Action id with `.` replaced by `_`; matches `TOOL_NAME_PATTERN` and is unique. */
  toolName: string
  actionId: string
  /** The action's own English description, unchanged (FR-001). */
  description: string
  inputSchema: JsonSchema
  effect: ActionDefinition['effect']
  /** Part of the fixed core offer every model sees (research R6). */
  core: boolean
  /** Localized titles, searched by `find_actions` together with id and description. */
  titles: Record<ActionLocale, string>
}

export type TitleOf = (titleKey: string, locale: ActionLocale) => string

/** Anthropic only accepts tool names of this shape (other providers are looser). */
export const TOOL_NAME_PATTERN = /^[A-Za-z0-9_-]{1,64}$/

/**
 * The fixed core offer: the same tools in every step for every model, local or cloud, chosen by
 * how often people ask for them, never from the text of a message (research R6). Together with
 * `find_actions` this stays at or under 10 tools. The first evaluation run (T046) confirms it.
 */
export const CORE_AGENT_TOOLS: readonly string[] = [
  'wm.state.get',
  'wm.apps.list',
  'wm.app.open',
  'wm.tab.new',
  'wm.tab.activate',
  'wm.tab.close',
  'settings.get',
  'settings.appearance.setColorScheme',
  'settings.models.list',
]

/** `wm.tab.back` → `wm_tab_back`. Ids use camelCase and no underscores, so this is reversible. */
export function toToolName(actionId: string): string {
  return actionId.replaceAll('.', '_')
}

/** Whether the built-in agent may be offered this action at all. */
export function isBuiltinAgentAction(def: ActionDefinition): boolean {
  return def.agentCallable && def.builtinAgentCallable !== false
}

/** Reverse lookup of `toToolName` over a catalog; `undefined` for names that are no action. */
export function fromToolName(
  toolName: string,
  catalog: readonly ActionDefinition[],
): ActionDefinition | undefined {
  return catalog.find((def) => toToolName(def.id) === toolName)
}

export function toAgentActionDef(
  def: ActionDefinition,
  titleOf: TitleOf,
): AgentActionDef {
  return {
    toolName: toToolName(def.id),
    actionId: def.id,
    description: def.description,
    inputSchema: def.input,
    effect: def.effect,
    core: CORE_AGENT_TOOLS.includes(def.id),
    titles: {
      de: titleOf(def.titleKey, 'de'),
      en: titleOf(def.titleKey, 'en'),
    },
  }
}

/** The definitions to push to Rust: only what the built-in agent may call (FR-001, FR-003). */
export function listAgentActions(
  catalog: readonly ActionDefinition[],
  titleOf: TitleOf,
): AgentActionDef[] {
  return catalog
    .filter(isBuiltinAgentAction)
    .map((def) => toAgentActionDef(def, titleOf))
}

/** What travels back to Rust after an action ran (`respond_action_call`, contracts/tauri-commands.md). */
export type ActionOutcomeWire =
  | { ok: true; result: unknown }
  | { ok: false; code: string; field?: string; message: string }

/**
 * The runner's outcome without the raw error a handler threw: that is for in-process callers and
 * never for a model (FR-006). Rust replaces the message of `failed` by a fixed text.
 */
export function toOutcomeWire(outcome: ActionOutcome): ActionOutcomeWire {
  if (outcome.ok) return { ok: true, result: outcome.result }
  return {
    ok: false,
    code: outcome.code,
    ...(outcome.field === undefined ? {} : { field: outcome.field }),
    message: outcome.message,
  }
}
