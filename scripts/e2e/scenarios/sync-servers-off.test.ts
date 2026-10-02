import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { unwrap } from '../lib/flows.ts'
import type { Device } from '../lib/group.ts'
import { expectOnline } from '../lib/group-expect.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'
import { openFederation, serverRows, toggleServers } from '../lib/sync-ui.ts'
import type { ScenarioContext } from '../lib/scenario.ts'

/** The Nostr servers of the settings (`SyncServersGroup`). */
const NOSTR = 'settings-servers-nostrRelays'

/** How long devices without a server are watched for finding each other. ponytail: a bounded wait is
 * no proof that they never will; the upgrade path is a counter of presence meetings in the app. */
const WATCH_MS = 15_000

async function switchAllNostrServers(
  ctx: ScenarioContext,
  device: Device,
  to: 'off' | 'on',
) {
  await openFederation(device.page)
  // The list fills in two steps (the built-in servers, then the vault's own); toggle only once it shows
  // every server the vault has.
  const defaults = unwrap<{ nostrRelays: string[] }>(
    'sync_servers_defaults',
    await device.page.invoke('sync_servers_defaults'),
  )
  const stored = unwrap<{ nostrRelays: string[] }>(
    'sync_servers_get',
    await device.page.invoke('sync_servers_get'),
  )
  const expected = new Set([...defaults.nostrRelays, ...stored.nostrRelays])
    .size
  await ctx.waitFor(
    `${device.address} to list its ${expected} Nostr servers`,
    async () => {
      return (
        (await serverRows(device.page, NOSTR, 'default')) +
          (await serverRows(device.page, NOSTR, 'added')) ===
        expected
      )
    },
  )
  // The setting belongs to the vault, so a change made on the other device may already have arrived
  // here: switch only what is not yet in the wanted state and check the result, not the clicks. The
  // built-in servers were off from the start and stay off; only the relay of the test comes back.
  if (to === 'off') {
    for (const kind of ['default', 'added'] as const) {
      await toggleServers(device.page, NOSTR, kind, 'on')
    }
  } else {
    await toggleServers(device.page, NOSTR, 'added', 'off')
  }
  await ctx.waitFor(
    `${device.address} to show its Nostr servers ${to}`,
    async () =>
      (await serverRows(device.page, NOSTR, 'default', true)) +
        (await serverRows(device.page, NOSTR, 'added', true)) ===
      (to === 'off' ? 0 : 1),
  )
}

// Spec 024, FR-008 (M9 of its quickstart): with every Nostr server switched off the devices no longer
// find each other, local work goes on (constitution VII), and with the servers on again they find each
// other and exchange what happened in between. The servers apply at the next opening of the vault.
scenario('sync-servers-off', { timeoutMs: 360_000 }, async (ctx) => {
  const g = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = g.device('anna/laptop')
  const phone = g.device('anna/phone')
  await expectOnline(ctx, laptop, phone, true)
  ctx.step('with the servers on, the devices find each other')

  for (const device of [laptop, phone]) {
    await switchAllNostrServers(ctx, device, 'off')
    await device.restart()
  }
  const end = Date.now() + WATCH_MS
  while (Date.now() < end) {
    for (const [observer, other] of [
      [laptop, phone],
      [phone, laptop],
    ] as const) {
      const row = (await observer.deviceList()).find((r) => !r.isCurrent)
      assert.ok(
        row !== undefined && row.online === false,
        `${observer.address} shows ${other.address} online with no server`,
      )
    }
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  ctx.step('with the servers off, they do not find each other')

  await addThread(laptop, 'nur am Laptop')
  await addThread(phone, 'nur am Telefon')
  assert.deepEqual(await threadTitles(laptop), ['nur am Laptop'])
  assert.deepEqual(await threadTitles(phone), ['nur am Telefon'])
  ctx.step('local work goes on without servers')

  for (const device of [laptop, phone]) {
    await switchAllNostrServers(ctx, device, 'on')
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
