import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { expectLastSeen, expectOnline } from '../lib/group-expect.ts'
import { deviceRows, openFederation } from '../lib/sync-ui.ts'

/** The promise of the online state (spec 024 user story 4, M4): a vanished device is shown as not
 * online within a minute (research R1). */
const NOTICED_WITHIN_MS = 60_000

// Spec 024, user story 4 (M4): the device view says which devices are reachable. A device that was
// stopped, and one that was killed without closing, is shown as "zuletzt online" with a matching
// time within 60 seconds; it is online again once it is back.
scenario('sync-presence', { timeoutMs: 360_000 }, async (ctx) => {
  const g = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = g.device('anna/laptop')
  const phone = g.device('anna/phone')
  await openFederation(laptop.page)

  const rowOfPhone = async () =>
    (await deviceRows(laptop.page)).find((row) => !row.current)

  await expectOnline(ctx, laptop, phone, true)
  await ctx.waitFor('the phone row to show online', async () => {
    const row = await rowOfPhone()
    return row?.online === true && row.status === 'online'
  })
  ctx.step('the phone is online')

  for (const how of ['stop', 'kill'] as const) {
    const before = Date.now()
    await (how === 'stop' ? phone.stop() : phone.kill())
    const row = await expectLastSeen(ctx, laptop, phone, {
      withinMs: 2 * NOTICED_WITHIN_MS,
      timeoutMs: NOTICED_WITHIN_MS,
    })
    const noticedAfter = Date.now() - before
    assert.ok(
      noticedAfter <= NOTICED_WITHIN_MS,
      `${how}: noticed after ${noticedAfter} ms`,
    )
    assert.ok(
      Math.abs((row.lastSeen ?? 0) - before) <= NOTICED_WITHIN_MS,
      `${how}: last seen ${row.lastSeen} does not match the time it ended (${before})`,
    )
    await ctx.waitFor(
      `the phone row to say "zuletzt online" after a ${how}`,
      async () => {
        const shown = await rowOfPhone()
        return (
          shown?.online === false &&
          shown?.status !== null &&
          shown?.status !== 'online' &&
          shown?.status !== 'Online'
        )
      },
    )
    ctx.step(`${how}: shown as last online after ${noticedAfter} ms`)
    await phone.start()
    await expectOnline(ctx, laptop, phone, true)
    ctx.step(`${how}: online again`)
  }
})
