// Password manager action catalog (spec 034-password-manager, FR-027, contracts/access.md): the
// one action the built-in agent may call, `passwords.items.search`. It returns the id, title, tag
// names, folder name and whether a TOTP exists of the matching entries, and nothing else: no
// username, no address, no secret. There is no action that reads, copies or changes a secret; the
// backend closes every other method to the agent as well, so this catalog is not the only guard.
// Pure, relative `.ts` imports only.
import type { ActionDefinition, JsonSchema } from './types.ts'

const SEARCH_INPUT: JsonSchema = {
  type: 'object',
  properties: {
    query: {
      type: 'string',
      description:
        'Words to look for in the title, the tag names and the folder name of an entry; every word must match.',
    },
    tag: {
      type: 'string',
      description: 'Only entries that carry this tag.',
    },
    limit: {
      type: 'integer',
      description: 'At most this many entries (at most 50, 20 by default).',
    },
  },
}

const SEARCH_RESULT: JsonSchema = {
  type: 'object',
  properties: {
    items: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          title: {
            type: 'string',
            description: 'Absent for an entry without a title.',
          },
          tags: { type: 'array', items: { type: 'string' } },
          folder: { type: 'string', description: 'Absent at the top level.' },
          hasTotp: { type: 'boolean' },
        },
      },
    },
  },
}

export const PASSWORDS_ACTIONS: readonly ActionDefinition[] = [
  {
    id: 'passwords.items.search',
    titleKey: 'actions.passwords.items.search',
    description:
      'Search the password manager by title, tag name or folder name. Returns id, title, tag names, folder name and whether the entry has a one-time code. It never returns usernames, addresses or any secret, and there is no way to read one.',
    input: SEARCH_INPUT,
    result: SEARCH_RESULT,
    target: 'none',
    scope: 'passwords.read',
    effect: 'read',
    agentCallable: true,
    binding: 'global',
  },
]

type RawHeader = {
  id?: unknown
  title?: unknown
  tags?: unknown
  folder?: unknown
  hasTotp?: unknown
}

/** The wire form of the backend's answer: exactly the five fields of the result schema, in that
 * shape, whatever else the answer carried. Absent title and folder are left out. */
export function projectSearchResult(raw: unknown): {
  items: {
    id: string
    title?: string
    tags: string[]
    folder?: string
    hasTotp: boolean
  }[]
} {
  const list = (raw as { items?: unknown } | null)?.items
  const items = Array.isArray(list) ? (list as RawHeader[]) : []
  return {
    items: items
      .filter((item) => typeof item?.id === 'string')
      .map((item) => ({
        id: item.id as string,
        ...(typeof item.title === 'string' ? { title: item.title } : {}),
        tags: Array.isArray(item.tags)
          ? item.tags.filter((tag): tag is string => typeof tag === 'string')
          : [],
        ...(typeof item.folder === 'string' ? { folder: item.folder } : {}),
        hasTotp: item.hasTotp === true,
      })),
  }
}
