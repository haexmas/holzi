// Helpers for the password manager scenarios (spec 034, T092): opening the window and reading what the
// backend holds through its commands, so a scenario checks data, not pixels. Entries are made through
// `passwords_create_item` where the editor is not what is being tested.
import { unwrap } from './flows.ts'
import { resizeAppWindow } from './settings.ts'
import type { FlowInstance } from './flows.ts'
import type { WaitContext } from './sync-flows.ts'

/** The overview as `passwords_load_overview` returns it, reduced to what the scenarios read. */
export interface Overview {
  headers: Array<{
    id: string
    title: string | null
    groupId: string | null
    tags: Array<{ id: string; name: string }>
  }>
  tags: Array<{ id: string; name: string; itemCount: number }>
}

/** Opens the password manager from the launcher, fits its window into the virtual screen (the
 * default geometry is wider than the 800 px screen, so the right-hand buttons would lie outside it)
 * and waits for its toolbar. */
export async function openPasswords(instance: FlowInstance): Promise<void> {
  await instance.click('open-launcher')
  await instance.click('[data-app-id="system.passwords"]')
  await instance.waitForDisplayed('passwords-search')
  await resizeAppWindow(instance, 'system.passwords', 760, 560)
  await instance.waitForDisplayed('passwords-new')
}

/** What the backend holds right now. */
export async function overview(
  instance: Pick<FlowInstance, 'invoke'>,
): Promise<Overview> {
  return unwrap<Overview>(
    'passwords_load_overview',
    await instance.invoke('passwords_load_overview'),
  )
}

/** Creates an entry through the command and returns its id. */
export async function createEntry(
  instance: Pick<FlowInstance, 'invoke'>,
  input: Record<string, unknown>,
): Promise<string> {
  const result = unwrap<{ itemId: string }>(
    'passwords_create_item',
    await instance.invoke('passwords_create_item', { args: { input } }),
  )
  return result.itemId
}

/** The titles of the entries the device holds, sorted. */
export async function entryTitles(
  instance: Pick<FlowInstance, 'invoke'>,
): Promise<string[]> {
  return (await overview(instance)).headers
    .map((header) => header.title ?? '')
    .sort()
}

/** Waits until the device holds exactly these entry titles (fixed deadline, as the sync waits). */
export async function expectEntries(
  ctx: WaitContext,
  instance: Pick<FlowInstance, 'invoke'>,
  titles: string[],
  description: string,
): Promise<void> {
  const wanted = [...titles].sort()
  await ctx.waitFor(
    description,
    async () => {
      const have = await entryTitles(instance)
      return JSON.stringify(have) === JSON.stringify(wanted) ? have : false
    },
    { timeoutMs: 40_000, fixed: true },
  )
}

/** How many tags the device holds whose name equals `name` ignoring case. */
export async function tagCount(
  instance: Pick<FlowInstance, 'invoke'>,
  name: string,
): Promise<number> {
  const wanted = name.toLowerCase()
  return (await overview(instance)).tags.filter(
    (tag) => tag.name.toLowerCase() === wanted,
  ).length
}

/** Moves an entry to the trash (`passwords_trash`) or, when it is in it already, deletes it for good. */
export async function trashEntry(
  instance: Pick<FlowInstance, 'invoke'>,
  id: string,
): Promise<void> {
  unwrap(
    'passwords_trash',
    await instance.invoke('passwords_trash', {
      args: { targets: [{ kind: 'item', id }] },
    }),
  )
}

/** Takes an entry out of the trash. */
export async function restoreEntry(
  instance: Pick<FlowInstance, 'invoke'>,
  id: string,
): Promise<void> {
  unwrap(
    'passwords_restore',
    await instance.invoke('passwords_restore', {
      args: { targets: [{ kind: 'item', id }] },
    }),
  )
}

/** The tab of an open entry (spec 036) as the tab bar marks it, or `null` when no tab bar shows. */
export async function activeTab(
  instance: FlowInstance,
): Promise<string | null> {
  return instance.exec<string | null>(
    `const on = document.querySelector('[data-testid^="entry-tab-"][data-state="active"]')
     return on ? on.getAttribute('data-testid').slice('entry-tab-'.length) : null`,
  )
}

/** Taps a tab of the open entry. */
export async function selectTab(
  instance: FlowInstance,
  tab: 'details' | 'extra' | 'history',
): Promise<void> {
  await instance.click(`entry-tab-${tab}`)
}

/** Whether the swipe surface shows this tab's slide: the slide follows the tab bar (spec 036 FR-002). */
export async function slideShows(
  instance: FlowInstance,
  tab: 'details' | 'extra' | 'history',
): Promise<boolean> {
  return instance.exec<boolean>(
    `const slide = document.querySelector('[data-testid="entry-panel-' + arguments[0] + '"]')
     return Boolean(slide && slide.classList.contains('swiper-slide-active'))`,
    [tab],
  )
}

/** Changes an entry through `passwords_update_item` (reading its update token first), so a scenario
 * can make states for the history without the editor. */
export async function updateEntry(
  instance: Pick<FlowInstance, 'invoke'>,
  id: string,
  patch: Record<string, unknown>,
): Promise<void> {
  const item = unwrap<{ updatedAt: string }>(
    'passwords_get_item',
    await instance.invoke('passwords_get_item', { args: { itemId: id } }),
  )
  unwrap(
    'passwords_update_item',
    await instance.invoke('passwords_update_item', {
      args: { itemId: id, expectedUpdatedAt: item.updatedAt, patch },
    }),
  )
}

/** The ids of the states of an entry, newest first, as the backend lists them. */
export async function historyIds(
  instance: Pick<FlowInstance, 'invoke'>,
  id: string,
): Promise<string[]> {
  return unwrap<Array<{ id: string }>>(
    'passwords_history_list',
    await instance.invoke('passwords_history_list', { args: { itemId: id } }),
  ).map((state) => state.id)
}
