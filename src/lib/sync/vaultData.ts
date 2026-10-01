// Who wants to hear that vault tables changed (spec 024 FR-032, for every writer): a registry of
// subscribers, each with the tables it shows. Pure — `useVaultData` feeds it from the
// `vault-data-changed` event, `scripts/check-vault-data.ts` tests it.

/** A table name, or a prefix ending in `*` (an extension's tables share `{key}__{name}__`). */
export type TablePattern = string

export interface VaultDataSubscription {
  tables: readonly TablePattern[]
  /** Reloads what the subscriber shows. Never runs twice at once: see {@link createVaultDataHub}. */
  handler: () => void | Promise<void>
}

export function matchesTable(pattern: TablePattern, table: string): boolean {
  return pattern.endsWith('*')
    ? table.startsWith(pattern.slice(0, -1))
    : pattern === table
}

/** Whether a change to any of `changed` concerns something shown from `patterns`. */
export function concerns(
  patterns: readonly TablePattern[],
  changed: readonly string[],
): boolean {
  return changed.some((table) =>
    patterns.some((pattern) => matchesTable(pattern, table)),
  )
}

interface Entry {
  subscription: VaultDataSubscription
  running: boolean
  again: boolean
}

/**
 * The subscribers of one vault session. A change reaches the subscribers whose tables it
 * touches. A subscriber's handler never overlaps itself: a change that arrives while it runs
 * makes it run once more afterwards, so the last state always wins and a burst costs at most
 * two reloads. A failing handler is reported to `onError` and does not stop the others.
 */
export function createVaultDataHub(onError: (error: unknown) => void) {
  const entries = new Set<Entry>()

  async function run(entry: Entry): Promise<void> {
    if (entry.running) {
      entry.again = true
      return
    }
    entry.running = true
    try {
      do {
        entry.again = false
        try {
          await entry.subscription.handler()
        } catch (error) {
          onError(error)
        }
      } while (entry.again && entries.has(entry))
    } finally {
      entry.running = false
    }
  }

  return {
    /** Returns the function that ends the subscription. */
    subscribe(subscription: VaultDataSubscription): () => void {
      const entry: Entry = { subscription, running: false, again: false }
      entries.add(entry)
      return () => {
        entries.delete(entry)
      }
    },
    /** Tells the subscribers concerned by `changed`; resolves when their handlers are done. */
    async dispatch(changed: readonly string[]): Promise<void> {
      const concerned = [...entries].filter((entry) =>
        concerns(entry.subscription.tables, changed),
      )
      await Promise.all(concerned.map(run))
    },
    get size(): number {
      return entries.size
    },
  }
}

export type VaultDataHub = ReturnType<typeof createVaultDataHub>
