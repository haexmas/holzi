import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { expectOnline } from '../lib/group-expect.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'

// The template for scenarios with several vaults (scripts/e2e/README.md): two users with two devices
// each. Anna's phone is stopped and started again; Ben's vault never shows what is Anna's, and the
// other way round. Devices are `<user>/<device>`; the first device of a user creates the vault.
scenario('sync-two-users', { timeoutMs: 480_000 }, async (ctx) => {
  const g = await ctx.group({
    users: { anna: ['laptop', 'phone'], ben: ['desktop', 'tablet'] },
  })
  const [laptop, phone] = [g.device('anna/laptop'), g.device('anna/phone')]
  const [desktop, tablet] = [g.device('ben/desktop'), g.device('ben/tablet')]

  // Stopped, not `goOffline`: a device without servers still reaches the others it knew directly.
  await phone.stop()
  await expectOnline(ctx, laptop, phone, false, 60_000)
  await addThread(laptop, 'Annas Chat')
  await addThread(desktop, 'Bens Chat')
  await expectThreads(
    ctx,
    tablet,
    ['Bens Chat'],
    "Ben's chat to reach his tablet",
  )
  ctx.step('the phone is gone, both vaults went on')

  await phone.start() // the phone finds the laptop and catches up
  await expectThreads(
    ctx,
    phone,
    ['Annas Chat'],
    "Anna's chat to reach her phone",
  )
  for (const device of [laptop, phone]) {
    assert.deepEqual(await threadTitles(device), ['Annas Chat'])
  }
  for (const device of [desktop, tablet]) {
    assert.deepEqual(await threadTitles(device), ['Bens Chat'])
  }
  ctx.step('after the restart each vault holds only its own chat')
})
