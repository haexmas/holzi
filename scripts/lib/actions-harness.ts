// Shared helpers for the action check scripts (`check-wm-actions.ts`, `check-agent-actions.ts`):
// a minimal valid input for a schema, and a runner over the shipped catalog whose handlers only
// record that they ran. Node type stripping: relative `.ts` imports, `import type`.
import { ALL_ACTIONS } from '../../src/lib/actions/catalog.ts'
import {
  createActionRunner,
  type ActionHandler,
} from '../../src/lib/actions/runner.ts'
import type { JsonSchema } from '../../src/lib/actions/types.ts'

/** The smallest value that satisfies a schema of the action subset. */
export function sample(schema: JsonSchema): unknown {
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

/** A runner over the real catalog; every handler pushes `'handled'` to `calls` and returns a
 * sample of the action's result schema. */
export function catalogRunner(calls: string[]) {
  const record =
    (id: string): ActionHandler =>
    () => {
      calls.push('handled')
      const definition = ALL_ACTIONS.find((action) => action.id === id)
      return definition ? sample(definition.result) : {}
    }
  return createActionRunner({
    catalog: ALL_ACTIONS,
    globalHandler: (id) => record(id),
    resolveFocus: () => 'focused',
    targetExists: () => true,
    awaitTabHandler: async (_appId, id) => record(id),
  })
}

/** The input field that names the target of an action. */
export const TARGET_FIELD = {
  tab: 'tabId',
  window: 'windowId',
  workspace: 'workspaceId',
} as const
