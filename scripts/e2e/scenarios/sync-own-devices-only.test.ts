import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import {
  addThread,
  expectThreads,
  linkDevice,
  startFirstDevice,
  threadTitles,
} from '../lib/sync-flows.ts'

// Spec 024, user story 2 (SC-005, SC-011): a device of another vault, running against the same Nostr
// relay at the same time, neither gets anything of this vault nor gives it anything. The two devices
// of this vault sync (the positive control: the sync had its time), and by then the foreign device still
// holds only its own.
scenario('sync-own-devices-only', { timeoutMs: 300_000 }, async (ctx) => {
  const relay = await ctx.nostrRelay()
  const mine = await startFirstDevice(ctx, relay.url, 'e2e-mine')
  const linked = await linkDevice(ctx, mine, relay.url, {
    deviceName: 'Mein zweites Gerät',
  })
  const foreign = await startFirstDevice(ctx, relay.url, 'e2e-theirs')
  const foreignPeer = await linkDevice(ctx, foreign, relay.url, {
    deviceName: 'Noch ein fremdes Gerät',
  })

  await addThread(foreign, 'nur beim Fremden')
  await addThread(mine, 'nur bei mir')
  await Promise.all([
    expectThreads(
      ctx,
      linked,
      ['nur bei mir'],
      'the thread to reach the own second device',
    ),
    expectThreads(
      ctx,
      foreignPeer,
      ['nur beim Fremden'],
      'the foreign thread to reach the foreign second device',
    ),
  ])
  ctx.step('both vaults complete a synchronization round')

  assert.deepEqual(
    await threadTitles(foreign),
    ['nur beim Fremden'],
    'the foreign device received something of this vault',
  )
  assert.deepEqual(await threadTitles(foreignPeer), ['nur beim Fremden'])
  assert.deepEqual(await threadTitles(mine), ['nur bei mir'])
  assert.deepEqual(await threadTitles(linked), ['nur bei mir'])
  ctx.step('the foreign device neither got nor gave anything')
})
