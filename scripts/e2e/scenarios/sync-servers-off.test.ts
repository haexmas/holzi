import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { unwrap } from '../lib/flows.ts'
import type { Device } from '../lib/group.ts'
import { expectOnline } from '../lib/group-expect.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'
import { openFederation, serverRows, toggleServers } from '../lib/sync-ui.ts'
import type { ScenarioContext } from '../lib/scenario.ts'

/** The server lists of the settings (`SyncServersGroup`). */
const LISTS = {
  nostrRelays: 'settings-servers-nostrRelays',
  irohRelays: 'settings-servers-irohRelays',
} as const
type List = keyof typeof LISTS

/** The application may keep a vanished peer's session for about 30 s; the spec promises 60 s. */
const NOTICED_WITHIN_MS = 60_000

/**
 * Switches every server of the vault off, or the test's own back on. That includes the iroh relays: a
 * device still knows the other's relay from before and would reach it there without any Nostr server
 * (a run with a phone meets through a local iroh relay; spec 043).
 */
async function switchAllServers(
  ctx: ScenarioContext,
  device: Device,
  to: 'off' | 'on',
) {
  for (const list of Object.keys(LISTS) as List[]) {
    await switchServers(ctx, device, list, to)
  }
}

async function switchServers(
  ctx: ScenarioContext,
  device: Device,
  list: List,
  to: 'off' | 'on',
) {
  const group = LISTS[list]
  await openFederation(device.page)
  // The list fills in two steps (the built-in servers, then the vault's own); toggle only once it shows
  // every server the vault has.
  const defaults = unwrap<Record<List, string[]>>(
    'sync_servers_defaults',
    await device.page.invoke('sync_servers_defaults'),
  )
  const stored = unwrap<Record<List, string[]>>(
    'sync_servers_get',
    await device.page.invoke('sync_servers_get'),
  )
  const expected = new Set([...defaults[list], ...stored[list]]).size
  await ctx.waitFor(
    `${device.address} to list its ${expected} ${list}`,
    async () => {
      return (
        (await serverRows(device.page, group, 'default')) +
          (await serverRows(device.page, group, 'added')) ===
        expected
      )
    },
  )
  // The setting belongs to the vault, so a change made on the other device may already have arrived
  // here: switch only what is not yet in the wanted state and check the result, not the clicks. The
  // built-in servers were off from the start and stay off; only the servers of the test come back.
  if (to === 'off') {
    for (const kind of ['default', 'added'] as const) {
      await toggleServers(device.page, group, kind, 'on')
    }
  } else {
    await toggleServers(device.page, group, 'added', 'off')
  }
  await ctx.waitFor(
    `${device.address} to show its ${list} ${to}`,
    async () =>
      (await serverRows(device.page, group, 'default', true)) +
        (await serverRows(device.page, group, 'added', true)) ===
      (to === 'off' ? 0 : 1),
  )
}

// Spec 024, FR-008 (M9 of its quickstart): with every server switched off the devices no longer find
// each other, local work goes on (constitution VII), and with the servers on again they find each
// other and exchange what happened in between. The servers apply at the next opening of the vault.
scenario('sync-servers-off', { timeoutMs: 360_000 }, async (ctx) => {
  const g = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = g.device('anna/laptop')
  const phone = g.device('anna/phone')
  await expectOnline(ctx, laptop, phone, true)
  ctx.step('with the servers on, the devices find each other')

  for (const device of [laptop, phone]) {
    await switchAllServers(ctx, device, 'off')
    await device.restart()
  }
  await expectOnline(ctx, laptop, phone, false, NOTICED_WITHIN_MS)
  await expectOnline(ctx, phone, laptop, false, NOTICED_WITHIN_MS)
  ctx.step('with the servers off, they do not find each other')

  await addThread(laptop, 'nur am Laptop')
  await addThread(phone, 'nur am Telefon')
  assert.deepEqual(await threadTitles(laptop), ['nur am Laptop'])
  assert.deepEqual(await threadTitles(phone), ['nur am Telefon'])
  ctx.step('local work goes on without servers')

  for (const device of [laptop, phone]) {
    await switchAllServers(ctx, device, 'on')
    await device.restart()
  }
  await expectOnline(ctx, laptop, phone, true)
  await expectOnline(ctx, phone, laptop, true)
  const both = ['nur am Laptop', 'nur am Telefon']
  await expectThreads(
    ctx,
    laptop,
    both,
    'the laptop to get the work of the phone',
  )
  await expectThreads(
    ctx,
    phone,
    both,
    'the phone to get the work of the laptop',
  )
  ctx.step(
    'with the servers on again, they find each other and exchange their work',
  )
})
