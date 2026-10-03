import { scenario } from '../lib/scenario.ts'
import { addThread, expectThreads } from '../lib/sync-flows.ts'

// Spec 024, user story 3 (SC-003, SC-011): a change reaches a device through another one. The first
// device writes, a second (a main device too) takes it over, the first goes away, and a third is
// linked through the second: what the first wrote reaches the third without the first running. When the
// first comes back, it and the third meet for the first time: nobody holds anything twice, and what each
// writes afterwards reaches the others.
scenario('sync-indirect', { timeoutMs: 480_000 }, async (ctx) => {
  const g = await ctx.group({
    users: { anna: ['a', { name: 'b', main: true }] },
  })
  const a = g.device('anna/a')
  const b = g.device('anna/b')

  await addThread(a, 'von A')
  await expectThreads(ctx, b, ['von A'], 'the thread of A to reach B')
  ctx.step('A to B')

  await a.stop()
  const c = await g.link(b, 'c')
  await expectThreads(
    ctx,
    c,
    ['von A'],
    'the thread of A to reach C through B while A is away',
  )
  ctx.step('A to C through B, A not running')

  await a.start()
  await addThread(c, 'von C')
  await expectThreads(
    ctx,
    a,
    ['von A', 'von C'],
    'A and C to meet and exchange without holding anything twice',
  )
  await addThread(a, 'von A nach der Rückkehr')
  const all = ['von A', 'von A nach der Rückkehr', 'von C']
  await expectThreads(ctx, c, all, 'what A writes after its return to reach C')
  await expectThreads(ctx, b, all, 'all three threads to reach B')
  ctx.step('all three hold the same threads, none twice')
})
