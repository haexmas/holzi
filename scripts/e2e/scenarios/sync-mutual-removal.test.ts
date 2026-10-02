import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import type { Device } from '../lib/group.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'
import {
  confirmRemoveDevice,
  exists,
  federationNotice,
  openFederation,
  openRemoveDevice,
} from '../lib/sync-ui.ts'

const AFTER = 'Nach der Einigung'

/** On `from`, removes `target` through the interface; the target is a main device, so the view warns. */
async function removeMainDevice(from: Device, target: Device) {
  await openRemoveDevice(from, target)
  assert.equal(
    await exists(from.page, 'remove-device-main-warning'),
    true,
    `${from.address}: removing a main device carries the warning`,
  )
  await confirmRemoveDevice(from.page)
}

// Spec 024, user story 6 (M7 of its quickstart, FR-029 to FR-031): two main devices that are not
// connected remove each other. Whoever wins, the vault ends with exactly one main device, the other one
// knows it was removed, and the linked device follows the winner. Which one wins is decided by the
// smaller hash and covered by the unit tests `on_a_tie_the_smallest_hash_wins` and
// `two_main_devices_removing_each_other_leave_exactly_one_main_device`; here either outcome is right.
scenario('sync-mutual-removal', { timeoutMs: 600_000 }, async (ctx) => {
  const g = await ctx.group({
    users: { anna: ['laptop', { name: 'desktop', main: true }, 'phone'] },
  })
  const [a, c, b] = ['laptop', 'desktop', 'phone'].map((name) =>
    g.device(`anna/${name}`),
  )
  assert.ok(a && b && c)
  const pubkeys = new Map<string, string>()
  for (const device of [a, b, c]) {
    pubkeys.set(device.address, await device.pubkey())
  }

  await a.goOffline()
  await c.goOffline()
  ctx.step('both main devices are cut off from each other')

  await removeMainDevice(a, c)
  await removeMainDevice(c, a)
  ctx.step('each removed the other')

  await a.goOnline()
  await c.goOnline()

  const outcome = await ctx.waitFor(
    'exactly one of the two main devices to stay main and the other to be removed',
    async () => {
      const states = [
        (await a.status()).thisDevice,
        (await c.status()).thisDevice,
      ]
      const settled =
        states.filter((state) => state === 'main').length === 1 &&
        states.filter((state) => state === 'removed').length === 1
      return settled ? { winner: states[0] === 'main' ? a : c } : false
    },
    { timeoutMs: 60_000, fixed: true },
  )
  const { winner } = outcome as { winner: Device }
  const loser = winner === a ? c : a
  ctx.step('one stays main, one is removed', `winner ${winner.address}`)

  await openFederation(loser.page)
  await loser.page.waitForDisplayed('settings-federation-notice')
  assert.ok((await federationNotice(loser.page)) !== null)

  const winnerKey = pubkeys.get(winner.address)
  const loserKey = pubkeys.get(loser.address)
  await ctx.waitFor(
    'the linked device to list the winner as its only main device',
    async () => {
      const mains = (await b.deviceList())
        .filter((row) => row.role === 'main')
        .map((row) => row.devicePubkey)
      return mains.length === 1 && mains[0] === winnerKey
    },
    { timeoutMs: 40_000, fixed: true },
  )
  assert.ok(
    !(await b.deviceList()).some((row) => row.devicePubkey === loserKey),
    'the linked device no longer lists the removed one',
  )
  ctx.step('the linked device follows the winner')

  await addThread(winner, AFTER)
  await expectThreads(ctx, b, [AFTER], 'the linked device to get the new chat')
  assert.deepEqual(
    await threadTitles(loser),
    [],
    'the removed main device gets nothing',
  )
  ctx.step('the winner and the linked device keep syncing')
})
