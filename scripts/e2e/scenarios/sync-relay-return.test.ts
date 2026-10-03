import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { expectOnline } from '../lib/group-expect.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'

/** How long devices may need to find each other again once the relay is back (gate G1 of spec 033). */
const RECONNECT_MS = 90_000

// Spec 033, edge case and gate G1: the test relay is switched off while devices run and comes back on the
// same address. The devices neither crash nor lose local work, and devices that started while the relay
// was away find each other once it is back.
scenario('sync-relay-return', { timeoutMs: 360_000 }, async (ctx) => {
  const g = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = g.device('anna/laptop')
  const phone = g.device('anna/phone')
  await expectOnline(ctx, laptop, phone, true)

  await g.relay.stop()
  assert.equal(g.relay.state, 'down')
  // A device reaches the other at the address it last had without any relay. Both start anew while the
  // relay is away, one after the other, so neither knows where the other is now and only the relay can
  // bring them together.
  await phone.stop()
  await laptop.restart()
  await phone.start()
  await addThread(laptop, 'während der Pause am Laptop')
  await addThread(phone, 'während der Pause am Telefon')
  assert.ok(
    laptop.page.alive() && phone.page.alive(),
    'both devices run without the relay',
  )
  assert.deepEqual(await threadTitles(laptop), ['während der Pause am Laptop'])
  ctx.step('without the relay the devices run and keep local work')

  await expectOnline(ctx, laptop, phone, false)
  await expectOnline(ctx, phone, laptop, false)
  ctx.step('without the relay they do not find each other')

  await g.relay.start()
  assert.equal(g.relay.state, 'up')
  const back = Date.now()
  await expectOnline(ctx, laptop, phone, true, RECONNECT_MS)
  ctx.step(
    'found each other again',
    `${Date.now() - back} ms after the relay returned`,
  )
  const both = ['während der Pause am Laptop', 'während der Pause am Telefon']
  await expectThreads(
    ctx,
    laptop,
    both,
    'the laptop to get the phone work from the pause',
  )
  await expectThreads(
    ctx,
    phone,
    both,
    'the phone to get the laptop work from the pause',
  )
})
