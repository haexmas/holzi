import { scenario } from '../lib/scenario.ts'
import {
  addThread,
  expectThreads,
  linkDevice,
  removeThread,
  renameThread,
  startFirstDevice,
} from '../lib/sync-flows.ts'

// Spec 024, user story 1 (SC-011): two devices of one vault, two application processes with data of
// their own, connect without anything entered and keep what the user creates, renames and deletes in
// step. The second device is linked the way a user does it (a code shown on the first). They find
// each other through a Nostr relay run for this scenario.
scenario('sync-two-devices', { timeoutMs: 240_000 }, async (ctx) => {
  const relay = await ctx.nostrRelay()
  const first = await startFirstDevice(ctx, relay.url, 'e2e-sync')
  const second = await linkDevice(ctx, first, relay.url, {
    deviceName: 'Zweitgerät',
  })

  const id = await addThread(first, 'vom ersten Gerät')
  await expectThreads(
    ctx,
    second,
    ['vom ersten Gerät'],
    'the new thread to show on the second device',
  )
  ctx.step('created on one device, shown on the other')

  await renameThread(second, id, 'vom zweiten umbenannt')
  await expectThreads(
    ctx,
    first,
    ['vom zweiten umbenannt'],
    'the new title to show on the first device',
  )
  ctx.step('renamed on the other, shown on the first')

  await removeThread(first, id)
  await expectThreads(
    ctx,
    second,
    [],
    'the deletion to reach the second device',
  )
  ctx.step('deleted on one, gone on the other')
})
