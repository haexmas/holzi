// Handler registries for the action runner (spec 020-tab-navigation, T015, research R19). Global
// handlers are registered once at startup; tab-bound handlers by a mounted app instance, and the
// runner can wait (with a timeout) for an app it just opened to register them.
import type { ActionHandler } from './runner.ts'

export type { ActionHandler, ActionHandlerContext } from './runner.ts'

type Waiter = {
  tabIds: ReadonlySet<string>
  actionId: string
  resolve: (handler: ActionHandler) => void
}

export function createHandlerRegistry() {
  const global = new Map<string, ActionHandler>()
  /** tabId → actionId → handler */
  const byTab = new Map<string, Map<string, ActionHandler>>()
  const waiters = new Set<Waiter>()

  function registerGlobal(actionId: string, handler: ActionHandler): void {
    global.set(actionId, handler)
  }

  function globalHandler(actionId: string): ActionHandler | undefined {
    return global.get(actionId)
  }

  /** Returns the unregister function (the app calls it on unmount). */
  function registerTab(
    tabId: string,
    actionId: string,
    handler: ActionHandler,
  ): () => void {
    const handlers = byTab.get(tabId) ?? new Map<string, ActionHandler>()
    handlers.set(actionId, handler)
    byTab.set(tabId, handlers)
    for (const waiter of [...waiters]) {
      if (waiter.actionId === actionId && waiter.tabIds.has(tabId)) {
        waiters.delete(waiter)
        waiter.resolve(handler)
      }
    }
    return () => {
      if (byTab.get(tabId)?.get(actionId) === handler)
        byTab.get(tabId)?.delete(actionId)
    }
  }

  function dropTab(tabId: string): void {
    byTab.delete(tabId)
  }

  function findTab(
    tabIds: Iterable<string>,
    actionId: string,
  ): ActionHandler | undefined {
    for (const tabId of tabIds) {
      const handler = byTab.get(tabId)?.get(actionId)
      if (handler) return handler
    }
    return undefined
  }

  /** The handler of `actionId` in any of `tabIds`, waiting up to `timeoutMs` for it. */
  function awaitTab(
    tabIds: readonly string[],
    actionId: string,
    timeoutMs: number,
  ): Promise<ActionHandler | null> {
    const found = findTab(tabIds, actionId)
    if (found) return Promise.resolve(found)
    return new Promise((resolve) => {
      const waiter: Waiter = {
        tabIds: new Set(tabIds),
        actionId,
        resolve: (handler) => {
          clearTimeout(timer)
          resolve(handler)
        },
      }
      const timer = setTimeout(() => {
        waiters.delete(waiter)
        resolve(null)
      }, timeoutMs)
      waiters.add(waiter)
    })
  }

  return {
    registerGlobal,
    globalHandler,
    registerTab,
    dropTab,
    findTab,
    awaitTab,
  }
}

export type HandlerRegistry = ReturnType<typeof createHandlerRegistry>
