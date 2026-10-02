import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'
import {
  confirmRemoveDevice,
  exists,
  federationNotice,
  openFederation,
  openRemoveDevice,
  removeConsequences,
} from '../lib/sync-ui.ts'

const BEFORE = 'Vor dem Entfernen'
const AFTER = 'Nach dem Entfernen'

// Spec 024, user story 6 (M6 of its quickstart, FR-026 to FR-028, FR-035): a main device removes another
// device through the interface, after a view that says what that means. The removed device is told, gets
// nothing new, and the devices that remain keep syncing.
scenario('sync-remove-device', { timeoutMs: 600_000 }, async (ctx) => {
  const g = await ctx.group({
    users: {
      anna: ['laptop', 'phone', { name: 'desktop', main: true }, 'tablet'],
    },
  })
  const [a, b, c, d] = ['laptop', 'phone', 'desktop', 'tablet'].map((name) =>
    g.device(`anna/${name}`),
  )
  assert.ok(a && b && c && d)
  await addThread(a, BEFORE)
  for (const device of [b, c, d]) {
    await expectThreads(
      ctx,
      device,
      [BEFORE],
      `${device.address} to get the first chat`,
    )
  }
  ctx.step('all four devices share a chat')

  await openRemoveDevice(a, d)
  const consequences = await removeConsequences(a.page)
  assert.equal(
    consequences.length,
    4,
    `the view states the four consequences: ${JSON.stringify(consequences)}`,
  )
  assert.equal(
    await exists(a.page, 'remove-device-main-warning'),
    false,
    'removing a linked device carries no warning about main devices',
  )
  assert.equal(
    (await d.deviceList()).length,
    4,
    'nothing is removed before it is confirmed',
  )
  ctx.step('the view says what removing means, nothing happened yet')

  await confirmRemoveDevice(a.page)
  await ctx.waitFor(
    'the removed device to learn that it was removed',
    async () => (await d.status()).thisDevice === 'removed',
    { timeoutMs: 40_000, fixed: true },
  )
  await openFederation(d.page)
  await d.page.waitForDisplayed('settings-federation-notice')
  assert.ok((await federationNotice(d.page)) !== null)
  ctx.step('the removed device says so')

  await addThread(a, AFTER)
  for (const device of [b, c]) {
    await expectThreads(
      ctx,
      device,
      [BEFORE, AFTER].sort(),
      `${device.address} to get the chat made after the removal`,
    )
  }
  // ponytail: absence has no event to wait for; the others have it, so a message to the removed device
  // would have been sent by now - the short wait gives it time to arrive anyway.
  await new Promise((resolve) => setTimeout(resolve, 5_000))
  assert.deepEqual(
    await threadTitles(d),
    [BEFORE],
    'the removed device receives nothing new',
  )
  ctx.step('the others keep syncing, the removed one gets nothing')
})
